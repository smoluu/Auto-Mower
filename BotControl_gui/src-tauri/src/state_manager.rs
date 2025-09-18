use std::{
    net::{ IpAddr, Ipv4Addr, UdpSocket },
    os::linux::raw::stat,
    sync::{ Arc, Mutex },
    time::{ Duration, Instant },
};
use serde::{ Deserialize, Serialize };
use tauri::{ AppHandle, Emitter, Manager };
use log::{ info, warn, error };
use std::thread;

// State manager holding all app state
#[derive(Clone,Serialize)]
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
            socket: Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:6969").unwrap())),
            sensor_data: None,
            settings: Settings {
                robot_address: Ipv4Addr::new(10, 66, 66, 5),
                robot_port: 6969,
                camera_url: String::from("rtsp://localhost:8554"),
            },
        }
    }
}
#[derive(Clone,Serialize)]
pub enum SensorData {}

#[derive(Clone,Deserialize, Serialize)]
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
    let state_arc = Arc::new(Mutex::new(state.clone()));
    let mut state_lock = state_arc.lock().map_err(|e| format!("failed to lock state {}", e))?;

    // check if already connected
    if matches!(state_lock.connection, ConnectionStatus::Connected | ConnectionStatus::Connecting) {
        return Err("Already connecting or connected".to_string());
    }
    state_lock.connection = ConnectionStatus::Connecting;

    let socket_lock = state_lock.socket.lock().unwrap();
    socket_lock.set_nonblocking(true).map_err(|e| {
        error!("Failed to set nonblocking: {}", e);
        format!("Failed to set nonblocking: {}", e)
    })?;
    socket_lock.connect(format!("{}:{}", address, port)).map_err(|e| {
        error!("Failed to connect to {}:{}: {}", address, port, e);
        format!("Failed to connect: {}", e)
    })?;
    app_handle.emit("state_connection_update", ConnectionStatus::Connecting).expect("Failed to emit state");
    info!("{}", format!("socket  {:?}", socket_lock));

    // Clone Arc for udp connection thread
    let state_arc_clone = state_arc.clone();
    let socket_arc_clone = state_lock.socket.clone();
    let app_handle_clone = app_handle.clone();

    thread::spawn(move || {
        info!("Started udp connection thread");
        let mut buffer = [0; 65536];
        let mut last_packet_time = Instant::now();

        loop {
            let mut state_lock = state_arc_clone.lock().unwrap();
            let socket_lock = socket_arc_clone.lock().unwrap();
            match socket_lock.recv(&mut buffer) {
                Ok(size) => {
                    last_packet_time = Instant::now();
                    info!("Received {} bytes", size);
                    app_handle_clone.emit("test", &buffer[0..size]).expect("Failed to emit data");
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if last_packet_time.elapsed() > Duration::from_secs(3) {
                        error!("No data received for 10 seconds");
                        state_lock.connection = ConnectionStatus::Disconnected;
                        app_handle_clone
                            .emit("state_connection_update", ConnectionStatus::Disconnected)
                            .expect("Failed to emit state");
                        break;
                    }
                }
                Err(e) => {
                    error!("Receive error: {}", e);
                    state_lock.connection = ConnectionStatus::Disconnected;
                    app_handle_clone
                        .emit("state_connection_update", ConnectionStatus::Disconnected)
                        .expect("Failed to emit state");
                    break;
                }
            }
            drop(state_lock);
            drop(socket_lock);
            thread::sleep(Duration::from_millis(10));
        }
    });

    Ok(())
}
