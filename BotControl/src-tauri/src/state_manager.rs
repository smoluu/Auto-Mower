use std::{
    net::{ IpAddr, Ipv4Addr, SocketAddr, UdpSocket },
    os::linux::raw::stat,
    sync::{ Arc, Mutex },
    time::{ Duration, Instant },
};
use serde::{ Deserialize, Serialize };
use tauri::{ AppHandle, Emitter, Manager };
use log::{ info, warn, error };
use std::thread;

const UDP_CONNECTION_TIMEOUT: Duration = Duration::from_millis(2000); // If no packets are received for this time, return from UDP connection thread

// State manager holding all app state
#[derive(Clone, Serialize)]
pub struct StateManager {
    pub connection: ConnectionStatus,
    #[serde(skip)]
    pub socket: Arc<Mutex<UdpSocket>>,
    pub sensor_data: Option<SensorData>,
    pub settings: Settings,
}

impl StateManager {
    pub fn new() -> Self {
        StateManager {
            connection: ConnectionStatus::Disconnected,
            socket: Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:0").unwrap())),
            sensor_data: None,
            settings: Settings {
                robot_address: Ipv4Addr::new(10, 66, 66, 50),
                robot_port: 6969,
                robot_mode: RobotMode::MANUAL,
                camera_url: String::from("rtsp://localhost:8554"),
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
pub enum RobotMode {
    MANUAL,
    AUTOMATIC,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Settings {
    robot_address: Ipv4Addr,
    robot_port: u32,
    robot_mode: RobotMode,
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
    let dest = "10.66.66.50:6969".parse::<SocketAddr>().unwrap();

    thread::spawn(move || {
        info!("Started udp connection thread");
        let mut state = state_arc_clone.lock().unwrap();
        let socket = socket_arc_clone.lock().unwrap();

        // Send ACK and listen for echo back
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

        drop(state);
        drop(socket);

        // Start Sending and Receiving data after ACK
        
        let mut buffer = [0; 65536];
        let mut last_packet_time = Instant::now();
        
        loop {
            let mut state_lock = state_arc_clone.lock().unwrap();
            let socket = socket_arc_clone.lock().unwrap();

            // Return if no packets are received for some time
            if last_packet_time.elapsed() > UDP_CONNECTION_TIMEOUT {
                error!("No data received for {} ms", UDP_CONNECTION_TIMEOUT.as_millis());
                state_lock.connection = ConnectionStatus::Disconnected;
                app_handle_clone
                    .emit("state_connection_update", ConnectionStatus::Disconnected)
                    .unwrap();
                return;
            }

            match socket.recv_from(&mut buffer) {
                Ok((len, src)) => {
                    last_packet_time = Instant::now();
                    info!("Received {:?} bytes", len);
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
            thread::sleep(Duration::from_millis(10));
        }
    });

    Ok(())
}
