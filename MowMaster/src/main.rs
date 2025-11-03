/*
This program handles main logic fo rthe robot.
UDP socket for fetching sensor data from esp32.
UDP socket that broadcasts processed sensor data to BotControl program.
bc short for BotControl
ms short for MowSense


*/

mod packets;
use serde::{ Serialize, Serializer };

use core::error;
use std::io::ErrorKind;
use std::net::{ Ipv4Addr, SocketAddr };
use std::str::FromStr;
use std::sync::{ Arc, Mutex, mpsc };
use std::thread::Thread;
use std::time::{ Duration, Instant };
use std::{ io::Error, net::UdpSocket };
use std::{ thread };

use log::{ debug, error, info };

fn main() -> Result<(), Error> {
    env_logger::init();

    const CONTROL_LOOP_INTERVAL: u64 = 20; // Milliseconds
    const BOTCONTROL_CONNECTION_LOOP_INTERVAL: u64 = 20;
    const MOWSENSE_CONNECTION_LOOP_INTERVAL: u64 = 20;

    // UDP socket for BotControl
    let botcontrol_socket = Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:6969")?));
    botcontrol_socket.lock().unwrap().set_nonblocking(true).expect("Could not enable nonblocking");

    // UDP socket for MowSense
    let mowsense_socket = Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:0")?));
    mowsense_socket.lock().unwrap().set_nonblocking(true).expect("Could not enable broadcast");

    // Mspc channel for sending data between control loop and MowSense UDP connection
    // Control loop to Mows sense
    let (cl_to_ms_tx, cl_to_ms_rx) = mpsc::channel::<Vec<u8>>();
    // MowSense to Control loop
    let (ms_to_cl_tx, ms_to_cl_rx) = mpsc::channel::<Vec<u8>>();

    // Mspc channel for sending data between control loop and BotControl UDP connection
    let (bc_to_cl_tx, bc_to_cl_rx) = mpsc::channel::<Vec<u8>>();

    // Control loop, receives data from BotControl and mowsense mpsc channels,
    thread::spawn(move || {
        let mut last_loop = Instant::now();

        loop {
            // Check for new data from BotControl mpsc channel
            match bc_to_cl_rx.try_recv() {
                Ok(data) => {
                    // Send straight to Mowmaster channel
                    let _ = cl_to_ms_tx.send(data);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {} // Skip if no data
                Err(e) => {
                    error!("CONTROL_LOOP -> Error trying to receive data from mpsc channel {}", e);
                }
            }
            // Check for new data from MowSense mpsc channel
            match ms_to_cl_rx.try_recv() {
                Ok(data) => {}
                Err(std::sync::mpsc::TryRecvError::Empty) => {} // Skip if no data
                Err(e) => {
                    error!("CONTROL_LOOP -> Error trying to receive data from mpsc channel {}", e);
                }
            }

            // Check for data from MowSense mpsc channel
        }
    });

    // Thread for handling Data from BotControl to control loop
    thread::spawn(move || {
        info!("Started BotControl UDP connection thread");

        let mut buf = [0u8; 1024];
        let mut last_keepalive = Instant::now();
        let mut dest: Option<SocketAddr> = None;
        let mut last_packet_recv = Instant::now();
        loop {
            let socket = botcontrol_socket.lock().unwrap();

            // Send keepalive every 1 second
            if
                last_keepalive.elapsed() >= Duration::from_secs(1) &&
                last_packet_recv.elapsed() <= Duration::from_secs(2) &&
                dest.is_some()
            {
                if let Err(e) = socket.send_to(b"KEEPALIVE", dest.unwrap()) {
                    error!("BOTCONTROL_SOCKET -> Could not send keepalive -> {}", e);
                } else {
                    info!("BOTCONTROL_SOCKET -> Sent keepalive");
                }
                last_keepalive = Instant::now();
            }

            // Receive data from BotControl, Send to Control loop
            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    info!("BOTCONTROL_SOCKET -> Received {len} bytes from {src}");
                    last_packet_recv = Instant::now();
                    // Echo ACK back
                    if &buf[..len] == b"ACK" {
                        dest = Some(src);
                        let _ = socket.send_to(&buf[..len], src);
                    } else {
                        // Send to Control loop
                        bc_to_cl_tx.send((&buf[..len]).to_vec()).unwrap();
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {} // Empty ignore
                Err(e) => {
                    error!("BOTCONTROL_SOCKET -> Could not read from BotControl -> {}", e);
                }
            }

            drop(socket);
            thread::sleep(Duration::from_millis(BOTCONTROL_CONNECTION_LOOP_INTERVAL));
        }
    });

    // Thread for handling MowSense UDP session
    thread::spawn(move || {
        info!("Started MowSense UDP connection thread");

        let mut buf = [0u8; 1024];
        let mut temp_buf = "NEEKERI".as_bytes();
        let dest = "10.26.180.161:6968".parse::<SocketAddr>().unwrap();
        let mut last_keepalive = Instant::now();

        loop {
            let socket = mowsense_socket.lock().unwrap();

            // Send keepalive every 1 second
            if last_keepalive.elapsed() >= Duration::from_secs(1) {
                if let Err(e) = socket.send_to(b"KEEPALIVE", &dest) {
                    error!("MOWSENSE_SOCKET -> Could not send keepalive -> {}", e);
                } else {
                    info!("MOWSENSE_SOCKET -> Sent keepalive");
                }
                last_keepalive = Instant::now();
            }

            // Receive data from control loop mpsc channel, Send to MowSense
            match cl_to_ms_rx.try_recv() {
                Ok(data) => {
                    // Data sending to MowSense
                    info!("ASDASDADADS data -> {:?}", &data);

                    match socket.send_to(&data, &dest) {
                        Ok(size) => {
                            info!("MOWSENSE_SOCKET -> Sent {:?} bytes to MowSense", size);
                        }
                        Err(e) => {
                            error!("MOWSENSE_SOCKET -> Could not write to MowSense -> {}", e);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {} // Empty ignore
                Err(e) => error!("MOWSENSE_SOCKET -> Channel receive error -> {}", e),
            }

            // Data receiving from MowSense,
            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    info!("MOWSENSE_SOCKET -> Received bytes -> {:?} from MowSense", &buf[..len]);
                    // send data to mspc
                    let _ = ms_to_cl_tx.send((&buf[..len]).to_vec());
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {} // Empty ignore
                Err(e) => {
                    error!("MOWSENSE_SOCKET -> Could not read from MowSense -> {}", e);
                }
            }

            drop(socket);
        }
    });

    // Fetch sensor data

    // Interpolate sensor data

    loop {
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}
