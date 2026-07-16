//! Hour 5 -- a minimal CLI to drive a node's behavior by hand: initialize a
//! data directory, submit transactions, produce and import blocks, replay
//! from storage, and inspect the result. Not a server, not a network peer
//! -- see the crate README for what each command does and an example
//! session.

mod cli;
mod commands;

fn main() {
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
