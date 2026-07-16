//! Hour 5 -- a minimal CLI to drive a node's behavior by hand: initialize a
//! data directory, submit transactions, produce and import blocks, replay
//! from storage, and inspect the result. Not a server, not a network peer
//! -- see the crate README for what each command does and an example
//! session.

mod cli;
mod commands;

/// Installs the process-wide `tracing` subscriber every library crate's
/// `info!`/`warn!` events (see `docs/logging.md`) get sent to. Defaults to
/// `info` level so the structured events this CLI is meant to demonstrate
/// are visible without extra setup; `RUST_LOG` overrides it the usual way
/// (e.g. `RUST_LOG=debug`, `RUST_LOG=storage=trace`).
fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn main() {
    init_logging();

    let args: Vec<String> = std::env::args().skip(1).collect();

    let command = match cli::parse(&args) {
        Ok(command) => command,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = commands::run(command) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
