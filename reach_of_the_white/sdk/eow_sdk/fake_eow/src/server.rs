use std::io;
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use crate::config::FakeEowConfig;
use crate::envelope::EventMetadataGenerator;
use crate::error::StartupError;
use crate::session;

pub fn run(config: FakeEowConfig) -> Result<(), StartupError> {
    let listener = TcpListener::bind(config.address).map_err(|source| StartupError::Bind {
        address: config.address,
        source,
    })?;
    let metadata = Arc::new(EventMetadataGenerator::default());

    println!(
        "[START] instance={} address={}",
        config.instance, config.address
    );

    loop {
        let (stream, peer) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(StartupError::Accept(error)),
        };
        println!("[CONNECT] peer={peer}");

        let instance = config.instance.clone();
        let metadata = Arc::clone(&metadata);
        thread::Builder::new()
            .name(format!("fake-eow-{peer}"))
            .spawn(move || {
                if let Err(error) = session::run(stream, peer, &instance, metadata) {
                    eprintln!("[ERROR] peer={peer} stage=session reason={error}");
                    println!("[CLOSE] peer={peer} reason=protocol_or_io_error");
                }
            })
            .map_err(StartupError::Spawn)?;
    }
}
