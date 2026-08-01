//! A deliberately small, independent stand-in for Echoes_of_the_White.

mod config;
mod envelope;
mod error;
mod frame;
mod handshake;
mod server;
mod session;

pub use config::FakeEowConfig;
pub use error::StartupError;
pub use server::ServerHandle;

/// Load the bundled `instance.json` and run the fake EOW server indefinitely.
pub fn run() -> Result<(), StartupError> {
    let config = FakeEowConfig::bundled()?;
    server::run(config)
}

/// Run the fake EOW server with an explicitly supplied configuration.
///
/// This entry point lets external integration harnesses start the blocking
/// server without duplicating its setup.
pub fn run_with_config(config: FakeEowConfig) -> Result<(), StartupError> {
    server::run(config)
}

/// Start a controllable fake EOW server. Port 0 selects an ephemeral port.
pub fn spawn_with_config(config: FakeEowConfig) -> Result<ServerHandle, StartupError> {
    server::spawn(config)
}
