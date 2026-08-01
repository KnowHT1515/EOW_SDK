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

/// Load the bundled `port.json` and run the fake EOW server indefinitely.
pub fn run() -> Result<(), StartupError> {
    let config = FakeEowConfig::bundled()?;
    server::run(config)
}

/// Run the fake EOW server with an explicitly supplied configuration.
///
/// This entry point exists so a future integration harness can start the
/// server without duplicating its setup. The current crate does not contain
/// any Transport-to-fake-EOW integration tests.
pub fn run_with_config(config: FakeEowConfig) -> Result<(), StartupError> {
    server::run(config)
}
