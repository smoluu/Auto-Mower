/*
This program handles main logic fo rthe robot.
UDP socket for fetching sensor data from esp32.
UDP socket that broadcasts processed sensor data to BotControl program.
bc short for BotControl
ms short for MowSense


*/

mod packets;
use packets::BotControlPacket;

use core::error;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::{ Arc, Mutex, mpsc };
use std::thread::Thread;
use std::time::Duration;
use std::{ io::Error, net::UdpSocket };
use std::{ thread };

use log::{ info, error };

fn main() -> Result<(), Error> {
    
    // UDP socket for BotControl
    let bc_socket = Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:6969")?));
    bc_socket.lock().unwrap().set_broadcast(true).expect("Could not enable broadcast");
    
    // UDP socket for MowSense
    let ms_socket = Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:6968")?));
    ms_socket.lock().unwrap().set_broadcast(true).expect("Could not enable broadcast");

    // Create mpsc channel for sending data from data handling thread to UDP session thread
    let (bc_send_tx, bc_send_rx) = mpsc::channel::<Vec<u8>>();
    // Create mpsc channel for sending data from UDP session thread to data handling thread
    let (bc_receive_tx, bc_receive_rx) = mpsc::channel::<Vec<u8>>();

    // Thread for handling BotControl UDP session
    thread::spawn(move || {
        let mut rx_buf = [0u8; 1024];
        loop {
            let socket = bc_socket.lock().unwrap();
            match socket.recv_from(&mut rx_buf) {
                Ok((len, src)) => {
                    info!("Received {len} bytes from {src}");
                    // Send avaivable data from mpsc channel to BotControl
                    match bc_send_rx.try_recv() {
                        Ok(data) => {
                            let len = socket.send_to(&data, src).unwrap();
                            info!("Sent {len} bytes to {src}");
                        }
                        Err(e) => {
                            error!("Error trying to receive data from mpsc channel {}", e);
                        }
                    }

                    // Send avaivable data from UDP session to command handler
                    match socket.recv_from(&mut rx_buf) {
                        Ok((len, src)) => {
                            bc_receive_tx.send(rx_buf.to_vec()).unwrap();
                        }
                        Err(e) => {
                            error!("Error receiving data {}", e);
                            continue;
                        }
                    }
                }
                Err(e) => {
                    return e;
                }
            }
            drop(socket);
            thread::sleep(Duration::from_millis(10));
        }
    });

    // Thread for handling MowSense UDP session
    thread::spawn(move || {
        let mut buf = [0u8; 1024]; 

        loop {
            let socket = ms_socket.lock().unwrap();
            socket.connect("192.168.69.123:6968").unwrap(); // set this from BotControl
            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    // handle data
                }
                Err(e) => {
                    error!("Could not read from MowSense -> {}", e);
                    continue;
                }
            }
            drop(socket);
            thread::sleep(Duration::from_millis(10));
        }
    });

    // Fetch sensor data

    // Interpolate sensor data

    Ok(())
}
