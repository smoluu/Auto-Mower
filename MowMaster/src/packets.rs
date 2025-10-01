use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Debug)]
pub struct BotControlPacket {
    timestamp: u64,
    
}