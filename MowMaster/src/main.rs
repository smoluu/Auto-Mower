/*
This program handles main logic fo rthe robot.
UDP socket for fetching sensor data from esp32.
UDP socket that broadcasts processed sensor data to BotControl program.



*/

use std::sync::{ Arc, Mutex, mpsc };
use std::{ io::Error, net::UdpSocket };
use std::thread;

fn main() -> Result<(), Error> {
    // create UDP broadcast socket for BotControl for robot initialization
    let socket = Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:6969")?));
    socket.lock().unwrap().set_broadcast(true).expect("Could enable broadcast");

    // Check if device is initialized

    // Spawn thread for accepting BotControl data stream
    let socket_clone = Arc::clone(&socket);
    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        loop {
            let socket_lock = socket_clone.lock().unwrap();
            match socket_lock.recv_from(&mut buf) {
                Ok((len, src)) => {
                    // spawn thread to send data to BotControl instance

                }
                Err(e) => {
                    return e;
                }
            }
        }
    });

    // Fetch sensor data

    // Interpolate sensor data

    Ok(())
}
