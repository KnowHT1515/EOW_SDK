use std::collections::HashSet;
use std::io;
use std::net::{SocketAddr, SocketAddrV4, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::config::FakeEowConfig;
use crate::envelope::EventMetadataGenerator;
use crate::error::StartupError;
use crate::session;

pub struct ServerHandle {
    address: SocketAddrV4,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<(), StartupError>>>,
}

impl ServerHandle {
    pub fn address(&self) -> SocketAddrV4 {
        self.address
    }

    pub fn stop(mut self) -> Result<(), StartupError> {
        self.stop.store(true, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker.join().map_err(|_| StartupError::WorkerPanicked)?,
            None => Ok(()),
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn run(config: FakeEowConfig) -> Result<(), StartupError> {
    let listener = bind(&config)?;
    serve_blocking(listener, config.instance)
}

pub fn spawn(config: FakeEowConfig) -> Result<ServerHandle, StartupError> {
    let listener = bind(&config)?;
    listener
        .set_nonblocking(true)
        .map_err(StartupError::LocalAddress)?;
    let address = local_v4(&listener)?;
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let instance = config.instance;
    let worker = thread::Builder::new()
        .name(format!("fake-eow-listener-{instance}"))
        .spawn(move || serve_until_stopped(listener, instance, worker_stop))
        .map_err(StartupError::Spawn)?;
    Ok(ServerHandle {
        address,
        stop,
        worker: Some(worker),
    })
}

fn bind(config: &FakeEowConfig) -> Result<TcpListener, StartupError> {
    TcpListener::bind(config.address).map_err(|source| StartupError::Bind {
        address: config.address,
        source,
    })
}

fn local_v4(listener: &TcpListener) -> Result<SocketAddrV4, StartupError> {
    match listener.local_addr().map_err(StartupError::LocalAddress)? {
        SocketAddr::V4(address) => Ok(address),
        SocketAddr::V6(_) => unreachable!("fake EOW always binds an IPv4 address"),
    }
}

fn serve_blocking(listener: TcpListener, instance: String) -> Result<(), StartupError> {
    let address = local_v4(&listener)?;
    println!("[START] instance={instance} address={address}");
    let metadata = Arc::new(EventMetadataGenerator::default());
    let active_sdk_ids = Arc::new(Mutex::new(HashSet::new()));

    loop {
        let (stream, peer) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(StartupError::Accept(error)),
        };
        spawn_session(
            stream,
            peer,
            &instance,
            Arc::clone(&metadata),
            Arc::clone(&active_sdk_ids),
        )?;
    }
}

fn serve_until_stopped(
    listener: TcpListener,
    instance: String,
    stop: Arc<AtomicBool>,
) -> Result<(), StartupError> {
    let address = local_v4(&listener)?;
    println!("[START] instance={instance} address={address}");
    let metadata = Arc::new(EventMetadataGenerator::default());
    let active_sdk_ids = Arc::new(Mutex::new(HashSet::new()));

    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, peer)) => spawn_session(
                stream,
                peer,
                &instance,
                Arc::clone(&metadata),
                Arc::clone(&active_sdk_ids),
            )?,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(StartupError::Accept(error)),
        }
    }
    Ok(())
}

fn spawn_session(
    stream: std::net::TcpStream,
    peer: SocketAddr,
    instance: &str,
    metadata: Arc<EventMetadataGenerator>,
    active_sdk_ids: Arc<Mutex<HashSet<String>>>,
) -> Result<(), StartupError> {
    stream
        .set_nonblocking(false)
        .map_err(StartupError::Accept)?;
    println!("[CONNECT] peer={peer}");
    let instance = instance.to_owned();
    thread::Builder::new()
        .name(format!("fake-eow-{peer}"))
        .spawn(move || {
            if let Err(error) = session::run(stream, peer, &instance, metadata, active_sdk_ids) {
                eprintln!("[ERROR] peer={peer} stage=session reason={error}");
                println!("[CLOSE] peer={peer} reason=protocol_or_io_error");
            }
        })
        .map_err(StartupError::Spawn)?;
    Ok(())
}
