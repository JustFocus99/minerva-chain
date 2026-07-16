//! Hand-rolled argument parsing -- no CLI framework, in keeping with the
//! rest of this workspace's dependency-light style (custom merkle root,
//! custom CRC32, deterministic placeholder signatures). Eight subcommands
//! with a handful of `--flag value` pairs each doesn't need one.

use std::path::PathBuf;

use primitives::{Amount, Nonce};

/// Every command defaults to this data directory unless `--data-dir` is
/// given, so a demo session only has to name it once, at `init`.
pub const DEFAULT_DATA_DIR: &str = "./data";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init {
        data_dir: PathBuf,
    },
    SubmitTx {
        data_dir: PathBuf,
        from: String,
        to: String,
        amount: Amount,
        nonce: Nonce,
    },
    ProduceBlock {
        data_dir: PathBuf,
    },
    ImportBlock {
        data_dir: PathBuf,
        block_path: PathBuf,
    },
    Replay {
        data_dir: PathBuf,
    },
    Tip {
        data_dir: PathBuf,
    },
    Accounts {
        data_dir: PathBuf,
    },
    ValidateStorage {
        data_dir: PathBuf,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CliError {
    #[error("no command given -- see the README for usage")]
    MissingCommand,
    #[error("unknown command {0:?} -- see the README for usage")]
    UnknownCommand(String),
    #[error("missing required --{0} flag")]
    MissingFlag(&'static str),
    #[error("--{flag} value {value:?} is not valid: {reason}")]
    InvalidFlag {
        flag: &'static str,
        value: String,
        reason: String,
    },
    #[error("missing required {0} argument")]
    MissingPositional(&'static str),
}

/// Splits `args` into `--flag value` pairs and everything else
/// (positional arguments). Every flag in this CLI takes a value -- there
/// are no bare boolean switches to special-case.
fn split_flags(args: &[String]) -> (Vec<(String, String)>, Vec<String>) {
    let mut flags = Vec::new();
    let mut positionals = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].strip_prefix("--") {
            Some(name) => {
                let value = args.get(i + 1).cloned().unwrap_or_default();
                flags.push((name.to_string(), value));
                i += 2;
            }
            None => {
                positionals.push(args[i].clone());
                i += 1;
            }
        }
    }

    (flags, positionals)
}

fn find_flag<'a>(flags: &'a [(String, String)], name: &str) -> Option<&'a str> {
    flags
        .iter()
        .find(|(flag, _)| flag == name)
        .map(|(_, value)| value.as_str())
}

fn require_flag(flags: &[(String, String)], name: &'static str) -> Result<String, CliError> {
    find_flag(flags, name)
        .map(str::to_string)
        .ok_or(CliError::MissingFlag(name))
}

fn parse_flag<T: std::str::FromStr>(
    flags: &[(String, String)],
    name: &'static str,
) -> Result<T, CliError>
where
    T::Err: std::fmt::Display,
{
    let raw = require_flag(flags, name)?;
    raw.parse().map_err(|err: T::Err| CliError::InvalidFlag {
        flag: name,
        value: raw.clone(),
        reason: err.to_string(),
    })
}

fn data_dir(flags: &[(String, String)]) -> PathBuf {
    PathBuf::from(find_flag(flags, "data-dir").unwrap_or(DEFAULT_DATA_DIR))
}

/// Parses `args` (already excluding the program name -- see `main.rs`)
/// into a [`Command`].
pub fn parse(args: &[String]) -> Result<Command, CliError> {
    let (command, rest) = args.split_first().ok_or(CliError::MissingCommand)?;
    let (flags, positionals) = split_flags(rest);
    let data_dir = data_dir(&flags);

    match command.as_str() {
        "init" => Ok(Command::Init { data_dir }),
        "submit-tx" => Ok(Command::SubmitTx {
            data_dir,
            from: require_flag(&flags, "from")?,
            to: require_flag(&flags, "to")?,
            amount: parse_flag(&flags, "amount")?,
            nonce: parse_flag(&flags, "nonce")?,
        }),
        "produce-block" => Ok(Command::ProduceBlock { data_dir }),
        "import-block" => Ok(Command::ImportBlock {
            data_dir,
            block_path: PathBuf::from(
                positionals
                    .first()
                    .ok_or(CliError::MissingPositional("block file path"))?,
            ),
        }),
        "replay" => Ok(Command::Replay { data_dir }),
        "tip" => Ok(Command::Tip { data_dir }),
        "accounts" => Ok(Command::Accounts { data_dir }),
        "validate-storage" => Ok(Command::ValidateStorage { data_dir }),
        other => Err(CliError::UnknownCommand(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_init_with_explicit_data_dir() {
        let command = parse(&args(&["init", "--data-dir", "./somewhere"])).unwrap();
        assert_eq!(
            command,
            Command::Init {
                data_dir: PathBuf::from("./somewhere"),
            }
        );
    }

    #[test]
    fn defaults_data_dir_when_omitted() {
        let command = parse(&args(&["tip"])).unwrap();
        assert_eq!(
            command,
            Command::Tip {
                data_dir: PathBuf::from(DEFAULT_DATA_DIR),
            }
        );
    }

    #[test]
    fn parses_submit_tx() {
        let command = parse(&args(&[
            "submit-tx",
            "--from",
            "alice",
            "--to",
            "bob",
            "--amount",
            "10",
            "--nonce",
            "0",
        ]))
        .unwrap();
        assert_eq!(
            command,
            Command::SubmitTx {
                data_dir: PathBuf::from(DEFAULT_DATA_DIR),
                from: "alice".to_string(),
                to: "bob".to_string(),
                amount: 10,
                nonce: 0,
            }
        );
    }

    #[test]
    fn submit_tx_requires_all_flags() {
        let err = parse(&args(&["submit-tx", "--from", "alice"])).unwrap_err();
        assert_eq!(err, CliError::MissingFlag("to"));
    }

    #[test]
    fn import_block_requires_positional_path() {
        let err = parse(&args(&["import-block"])).unwrap_err();
        assert_eq!(err, CliError::MissingPositional("block file path"));
    }

    #[test]
    fn import_block_parses_positional_path() {
        let command = parse(&args(&["import-block", "./block.bin"])).unwrap();
        assert_eq!(
            command,
            Command::ImportBlock {
                data_dir: PathBuf::from(DEFAULT_DATA_DIR),
                block_path: PathBuf::from("./block.bin"),
            }
        );
    }

    #[test]
    fn rejects_unknown_command() {
        let err = parse(&args(&["frobnicate"])).unwrap_err();
        assert_eq!(err, CliError::UnknownCommand("frobnicate".to_string()));
    }

    #[test]
    fn rejects_missing_command() {
        let err = parse(&args(&[])).unwrap_err();
        assert_eq!(err, CliError::MissingCommand);
    }
}
