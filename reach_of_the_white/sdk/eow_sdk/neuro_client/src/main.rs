use std::error::Error;
use std::sync::mpsc;

use neuro_adapter_to_neuro_sdk::{E2naServer, ServerConfig};

const NEURO_QUEUE_CAPACITY: usize = 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let (neuro_sender, neuro_queue) = mpsc::sync_channel(NEURO_QUEUE_CAPACITY);
    let server = E2naServer::bind(ServerConfig::default(), neuro_sender)?;
    println!("E2NA listening on {}", server.local_address());

    for batch in neuro_queue {
        for message in batch.messages {
            println!("{message}");
        }
    }

    server.stop()?;
    Ok(())
}
