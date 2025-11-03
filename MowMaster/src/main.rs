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
    mowsense_socket.lock().unwrap().set_broadcast(true).expect("Could not enable broadcast");

    // Mspc channel for sending data between control loop and mow sense UDP connection
    let (control_loop_ms_tx, control_loop_ms_rx) = mpsc::channel::<Vec<u8>>();

    // Mspc channel for sending data between control loop and BotControl UDP connection
    let (control_loop_bc_tx, control_loop_bc_rx) = mpsc::channel::<Vec<u8>>();

    // Control loop, receives data from BotControl and mowsense mpsc channels,
    thread::spawn(move || {
        let mut last_loop = Instant::now();

        loop {
            if last_loop.elapsed() >= Duration::from_millis(CONTROL_LOOP_INTERVAL) {
                continue;
            }
            // Check for new data from BotControl mpsc channel
            match control_loop_bc_rx.try_recv() {
                Ok(data) => {
                    info!("CONTROL_LOOP -> Received {} bytes from ", data.len());
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {} // Skip if no data
                Err(e) => {
                    error!("BOTCONTROL_SOCKET -> Error trying to receive data from mpsc channel {}", e);
                }
            }

            // Check for data from MowSense mpsc channel

            last_loop = Instant::now();
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
                // Here we broadcast to a default address or some known BotControl address
                // If you have a specific destination, replace with that
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
                        control_loop_bc_tx.send((&buf[..len]).to_vec()).unwrap();
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
            match control_loop_ms_rx.try_recv() {
                Ok(data) => {
                    // Data sending to MowSense
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
                Err(e) => error!("BOTCONTROL_SOCKET -> Channel receive error -> {}", e),
            }

            // Data receiving from MowSense,
            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    info!("MOWSENSE_SOCKET -> Received bytes -> {:?}", &buf[..len]);
                    // send data to mspc
                    let _ = control_loop_ms_tx.send((&buf[..len]).to_vec());
                }
                Err(e) => {
                    error!("MOWSENSE_SOCKET -> Could not read from MowSense -> {}", e);
                }
            }

            drop(socket);
            thread::sleep(Duration::from_millis(MOWSENSE_CONNECTION_LOOP_INTERVAL));
        }
    });

    // Fetch sensor data

    // Interpolate sensor data

    loop {
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}
