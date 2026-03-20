use crate::gamepad::{ self, ControlInputs, RobotMode };
use crate::packets::Packet;
use log::{ debug, error, info, warn };
use serde::{ Deserialize, Serialize };
use tauri::http::uri::Port;
use std::fmt::format;
use std::thread;
use std::{
    net::{ IpAddr, Ipv4Addr, SocketAddr, UdpSocket },
    os::linux::raw::stat,
    sync::{ mpsc::{ self, Receiver, Sender, TryRecvError }, Arc, Mutex },
    time::{ Duration, Instant },
};
use tauri::{ App, AppHandle, Emitter, Manager };
const UDP_CONNECTION_TIMEOUT: Duration = Duration::from_millis(5000); // If no packets are received for this time, return from UDP connection thread

// State manager holding all app state
#[derive(Clone)]
pub struct StateManager {
    pub connection: ConnectionStatus,
    pub socket: Arc<Mutex<UdpSocket>>,
    pub udp_tx: Option<Sender<&'static [u8]>>,
    pub sensor_data: Option<SensorData>,
    pub settings: Settings,
    pub control_inputs: ControlInputs,
}

impl StateManager {
    pub fn new() -> Self {
        StateManager {
            connection: ConnectionStatus::Disconnected,
            socket: Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:0").unwrap())),
            udp_tx: None,
            sensor_data: None,
            settings: Settings {
                robot_address: Ipv4Addr::new(10, 66, 66, 50),
                robot_port: 6969,
                camera_url: String::from("rtsp://localhost:8554"),
            },
            control_inputs: ControlInputs {
                throttle: 0.0,
                steering: 0.0,
                mode: RobotMode::MANUAL,
                mower_ena: 0,
                mower_speed: 0.0,
            },
        }
    }
}

#[derive(Clone, Serialize)]
pub enum SensorData {}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Settings {
    robot_address: Ipv4Addr,
    robot_port: u32,
    camera_url: String,
}

pub type AppState = StateManager;

#[tauri::command]
pub fn connect_udp(address: String, port: u32, app_handle: AppHandle) -> Result<(), String> {
    let state = app_handle.state::<AppState>().inner();
    // check if already connected
    if matches!(state.connection, ConnectionStatus::Connected | ConnectionStatus::Connecting) {
        return Err("Already connecting or connected".to_string());
    }
    app_handle
        .emit("state_connection_update", ConnectionStatus::Connecting)
        .expect("Failed to emit state");
    let state_arc = Arc::new(Mutex::new(state.clone()));
    let mut state_lock = state_arc.lock().map_err(|e| format!("failed to lock state {}", e))?;
    state_lock.connection = ConnectionStatus::Connecting;

    let socket = state_lock.socket.lock().unwrap();
    socket.set_nonblocking(true).map_err(|e| {
        error!("Failed to set nonblocking: {}", e);
        format!("Failed to set nonblocking: {}", e)
    })?;

    info!("{}", format!("socket  {:?}", socket));

    // Clone Arc for udp connection thread
    let state_arc_clone = state_arc.clone();
    let socket_arc_clone = state_lock.socket.clone();
    let app_handle_clone = app_handle.clone();

    let dest_string = format!("{}:{}", address, port);
    let dest = dest_string.parse::<SocketAddr>().unwrap();

    thread::spawn(move || {
        info!("Started udp connection thread");
        let mut state = state_arc_clone.lock().unwrap();
        let socket = socket_arc_clone.lock().unwrap();

        // Send ACK and listen for echo back
        info!("Trying to send ACK to {dest}");
        match socket.send_to(b"ACK", &dest) {
            Ok(num_of_bytes) => {
                info!("Sent {num_of_bytes} to MowMaster");
            }
            Err(e) => {
                error!("Error sending ACK to -> {} -> {}", dest, e);
            }
        }
        let _ = socket.set_nonblocking(false);
        let _ = socket.set_read_timeout(Some(Duration::from_millis(1000)));
        let _ = socket.set_write_timeout(Some(Duration::from_millis(1000)));
        let mut buf = [0; 128];
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                if &buf[..len] != b"ACK" {
                    warn!("Handshake failed – expected ACK, got {len} bytes from {src}");
                    state.connection = ConnectionStatus::Disconnected;
                    app_handle_clone
                        .emit("state_connection_update", ConnectionStatus::Disconnected)
                        .unwrap();
                    return;
                }
                info!("Handshake OK – received ACK from {src}");
            }
            Err(e) => {
                error!("Handshake recv error: {e}");
                state.connection = ConnectionStatus::Disconnected;
                app_handle_clone
                    .emit("state_connection_update", ConnectionStatus::Disconnected)
                    .unwrap();
                return;
            }
        }

        let _ = socket.set_nonblocking(true);
        state.connection = ConnectionStatus::Connected;
        app_handle_clone.emit("state_connection_update", ConnectionStatus::Connected).unwrap();

        let (udp_tx, udp_rx) = mpsc::channel::<&'static [u8]>();
        state.udp_tx = Some(udp_tx.clone());

        drop(state);
        drop(socket);

        // Start Control input thread
        let handle = thread::spawn(move || {
            ControlInputs::start(udp_tx.clone());
        });

        // Start Sending and Receiving data after ACK

        let mut buffer = [0; 65536];
        let mut last_packet_recv_time = Instant::now();
        let mut last_loop = Instant::now();
        let mut last_keepalive = Instant::now();
        let mut dest: Option<SocketAddr> = None;

        loop {
            while last_loop.elapsed() < Duration::from_millis(1) {
                std::thread::yield_now();
            }
            let mut state_lock = state_arc_clone.lock().unwrap();
            let socket = socket_arc_clone.lock().unwrap();

            // Send keepalive every 1 second
            if last_keepalive.elapsed() >= Duration::from_secs(1) && dest.is_some() {
                // Here we broadcast to a default address or some known BotControl address
                // If you have a specific destination, replace with that
                if let Err(e) = socket.send_to(b"KEEPALIVE", dest.unwrap()) {
                    error!("BOTCONTROL_SOCKET -> Could not send keepalive -> {}", e);
                } else {
                    info!("BOTCONTROL_SOCKET -> Sent keepalive");
                }
                last_keepalive = Instant::now();
            }

            // Return if no packets are received for some time
            if last_packet_recv_time.elapsed() > UDP_CONNECTION_TIMEOUT {
                error!("No data received for {} ms", UDP_CONNECTION_TIMEOUT.as_millis());
                state_lock.connection = ConnectionStatus::Disconnected;
                app_handle_clone
                    .emit("state_connection_update", ConnectionStatus::Disconnected)
                    .unwrap();
                // Reset socket
                state_lock.socket = Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:0").unwrap()));
                return;
            }

            // Read data from mpsc channel, Send to MowMaster
            match udp_rx.try_recv() {
                Ok(data) => {
                    if Some(dest).is_some() {
                        let _ = socket.send_to(&data, dest.unwrap());
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    error!("MPSC Channel disconnected");
                }
            }

            match socket.recv_from(&mut buffer) {
                Ok((len, src)) => {
                    dest = Some(src);
                    last_packet_recv_time = Instant::now();
                    info!("Received {:?} bytes", len);

                    // Parse received packets

                    if let Some(packet) = Packet::parse(&buffer[..len]) {
                        match &packet {
                            Packet::PNTT {
                                heading,
                                roll,
                                pitch,
                                temp_c_0,
                                acc_total,
                                pressure,
                                temp_c_1,
                                timestamp_us,
                            } => {
                                // Emit to frontend telemetry handler
                                app_handle_clone.emit("TELEMETRY", &packet).unwrap();
                                debug!(
                                    "PNTT -> heading={:+.2}°, roll={:+.2}°, pitch={:+.2}°, temp_0={:+.2}°C, acc={:+.2}g, press={:.1}, temp_1={:.2}, Pa, ts={}",
                                    heading,
                                    roll,
                                    pitch,
                                    temp_c_0,
                                    acc_total,
                                    pressure,
                                    temp_c_1,
                                    timestamp_us
                                );
                                
                            }
                            Packet::Keepalive {} => {} // ignore
                            _ => {
                                warn!("Packet malformed");
                            }
                        }
                    }

                    app_handle_clone.emit("test", &buffer[0..len]).expect("Failed to emit data");
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    error!("Receive error: {}", e);
                    state_lock.connection = ConnectionStatus::Disconnected;
                    app_handle_clone
                        .emit("state_connection_update", ConnectionStatus::Disconnected)
                        .unwrap();
                    break;
                }
            }
            drop(state_lock);
            drop(socket);
            last_loop = Instant::now();
        }
    });

    Ok(())
}
