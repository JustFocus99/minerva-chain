//! What each CLI command actually does. Every command re-derives chain
//! state from disk (`Chain::from_storage`, `docs/replay.md`) rather than
//! keeping anything in memory between invocations -- there is no daemon
//! here, each `minerva-node` run is its own short-lived process.

use std::path::{Path, PathBuf};

use block::block::{Block, BlockHeader};
use block::{GENESIS_PARENT_HASH, merkle_root};
use chain::Chain;
use execution::GenesisConfig;
use mempool::pool::{PoolAdmission, TransactionPool};
use primitives::{AccountId, Amount, Nonce};
use state::chain_state::ChainState;
use storage::{AppendOnlyBlockStore, BlockStore, record};
use transaction::transaction::{SignedTransaction, UnsignedTransaction};

use crate::cli::Command;

/// The demo genesis every command derives independently: fixed named
/// accounts, deterministically derived from their names by hashing (see
/// `account_id_for_name`), each seeded with the same starting balance.
/// This is a fixed, explicit starting point in the same sense
/// `docs/replay.md`'s `GenesisConfig` requires -- nothing here is inferred
/// from the block log, and every command that rebuilds state from storage
/// (`tip`, `accounts`, `produce-block`, `import-block`, `replay`) uses
/// exactly this genesis, so two separate `minerva-node` invocations only
/// ever agree if they started from the same one.
const DEMO_ACCOUNT_NAMES: [&str; 3] = ["alice", "bob", "carol"];
const FEE_COLLECTOR_NAME: &str = "fee-collector";
const DEMO_STARTING_BALANCE: Amount = 1_000_000;

/// The block log's fixed producer id -- there is no validator selection or
/// proof-of-work here, just one deterministic placeholder producer, since
/// this CLI only demonstrates single-node block production.
fn producer_id() -> AccountId {
    account_id_for_name("minerva-node")
}

/// Deterministically derives a 32-byte account id from a human-readable
/// name, so `--from alice` always resolves to the same account across
/// every command and every run -- no address book or key file to keep in
/// sync.
fn account_id_for_name(name: &str) -> AccountId {
    crypto::hash::hash_bytes(name.as_bytes())
}

fn demo_genesis() -> GenesisConfig {
    let fee_collector = account_id_for_name(FEE_COLLECTOR_NAME);
    let accounts = DEMO_ACCOUNT_NAMES
        .iter()
        .map(|name| (account_id_for_name(name), DEMO_STARTING_BALANCE))
        .chain(std::iter::once((fee_collector, 0)))
        .collect();
    GenesisConfig::new(accounts, fee_collector)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage error: {0}")]
    Storage(#[from] storage::StorageError),
    #[error("node startup (recover + replay) failed: {0}")]
    Startup(#[from] chain::StartupError),
    #[error("block import failed: {0}")]
    Import(#[from] chain::ImportError),
    #[error("replay failed: {0}")]
    Replay(#[from] execution::ReplayError),
    #[error("state error: {0}")]
    State(#[from] state::error::StateError),
    #[error("node data directory {0:?} is not initialized -- run `minerva-node init` first")]
    NotInitialized(PathBuf),
    #[error(
        "invalid transaction: {from} -> {to} amount {amount} (amount must be non-zero and sender must differ from receiver)"
    )]
    InvalidTransaction {
        from: String,
        to: String,
        amount: Amount,
    },
    #[error(
        "mempool file is corrupt: length {len} is not a multiple of the fixed transaction record size {record_len}"
    )]
    CorruptMempoolFile { len: usize, record_len: usize },
    #[error(
        "{path:?} contains {extra} trailing byte(s) after one block record -- import-block only accepts a single-block file"
    )]
    TrailingBytesInBlockFile { path: PathBuf, extra: usize },
}

pub fn run(command: Command) -> Result<(), CommandError> {
    match command {
        Command::Init { data_dir } => run_init(&data_dir),
        Command::SubmitTx {
            data_dir,
            from,
            to,
            amount,
            nonce,
        } => run_submit_tx(&data_dir, from, to, amount, nonce),
        Command::ProduceBlock { data_dir } => run_produce_block(&data_dir),
        Command::ImportBlock {
            data_dir,
            block_path,
        } => run_import_block(&data_dir, &block_path),
        Command::Replay { data_dir } => run_replay(&data_dir),
        Command::Tip { data_dir } => run_tip(&data_dir),
        Command::Accounts { data_dir } => run_accounts(&data_dir),
        Command::ValidateStorage { data_dir } => run_validate_storage(&data_dir),
    }
}

fn blocks_log_path(data_dir: &Path) -> PathBuf {
    data_dir.join("blocks.log")
}

fn mempool_path(data_dir: &Path) -> PathBuf {
    data_dir.join("mempool.dat")
}

fn ensure_initialized(data_dir: &Path) -> Result<(), CommandError> {
    if blocks_log_path(data_dir).exists() {
        Ok(())
    } else {
        Err(CommandError::NotInitialized(data_dir.to_path_buf()))
    }
}

fn open_store(data_dir: &Path) -> Result<AppendOnlyBlockStore, CommandError> {
    Ok(AppendOnlyBlockStore::open(blocks_log_path(data_dir))?)
}

/// Rebuilds a `Chain` entirely from durable storage -- see
/// `chain::Chain::from_storage` / `docs/replay.md`. Every command that
/// needs "the current state of the node" goes through this, never through
/// state carried over from a previous invocation.
fn load_chain(data_dir: &Path) -> Result<Chain<AppendOnlyBlockStore>, CommandError> {
    let store = open_store(data_dir)?;
    Ok(Chain::from_storage(demo_genesis(), store)?)
}

fn run_init(data_dir: &Path) -> Result<(), CommandError> {
    std::fs::create_dir_all(data_dir)?;
    // Opening (and immediately dropping) the store creates blocks.log if
    // it doesn't already exist -- that file's presence is what
    // `ensure_initialized` checks for.
    let _store = open_store(data_dir)?;
    println!("initialized node data directory at {}", data_dir.display());
    Ok(())
}

fn run_submit_tx(
    data_dir: &Path,
    from: String,
    to: String,
    amount: Amount,
    nonce: Nonce,
) -> Result<(), CommandError> {
    ensure_initialized(data_dir)?;

    let unsigned = UnsignedTransaction {
        from: account_id_for_name(&from),
        to: account_id_for_name(&to),
        amount,
        nonce,
    };
    if !unsigned.is_valid() {
        return Err(CommandError::InvalidTransaction { from, to, amount });
    }
    let tx = SignedTransaction::sign(unsigned);

    mempool_store::append(&mempool_path(data_dir), &tx)?;

    println!(
        "queued tx {}: {from} -> {to} amount {amount} nonce {nonce}",
        hex(tx.transaction.id().as_bytes())
    );
    Ok(())
}

/// Builds a candidate block on top of `state` from `transactions`, computing
/// the transaction root and resulting state root the same way
/// `ChainState::execute_block` will independently re-derive and check them
/// -- see `docs/block-validation.md`. `transactions` is assumed to already
/// be nonce-ordered and admissible (the caller, `run_produce_block`, gets
/// that from `mempool::TransactionPool`); this does not re-implement pool
/// admission.
fn build_block(state: &ChainState, transactions: Vec<SignedTransaction>) -> Result<Block, CommandError> {
    let tx_ids = transactions
        .iter()
        .map(|tx| tx.transaction.id())
        .collect::<Vec<_>>();
    let transaction_root = merkle_root(&tx_ids);

    let mut candidate = state.clone();
    for tx in &transactions {
        candidate.apply_signed_transaction(tx.clone())?;
    }
    let state_commitment = candidate.state_commitment();

    let (height, parent_hash) = match state.tip() {
        None => (0, GENESIS_PARENT_HASH),
        Some(tip) => (tip.height + 1, tip.block_hash),
    };

    // No wall clock in this codebase's determinism model (docs/architecture.md)
    // -- height doubles as the slot number.
    let header = BlockHeader::new(height, parent_hash, transaction_root, state_commitment, producer_id(), height);
    Ok(Block { header, transactions })
}

fn run_produce_block(data_dir: &Path) -> Result<(), CommandError> {
    ensure_initialized(data_dir)?;

    let mut chain = load_chain(data_dir)?;
    let mempool_file = mempool_path(data_dir);
    let pending = mempool_store::read(&mempool_file)?;

    let mut pool = TransactionPool::new();
    let mut dropped = 0usize;
    for tx in pending {
        match pool.submit_transaction(tx, chain.state()) {
            PoolAdmission::Accepted | PoolAdmission::QueuedForFutureNonce => {}
            PoolAdmission::Duplicate | PoolAdmission::Rejected(_) => dropped += 1,
        }
    }

    let ready: Vec<SignedTransaction> = pool
        .ready_transactions(chain.state())
        .into_iter()
        .cloned()
        .collect();
    for tx in &ready {
        if let Some(by_nonce) = pool.transactions.get_mut(&tx.transaction.from) {
            by_nonce.remove(&tx.transaction.nonce);
        }
    }

    let block = build_block(chain.state(), ready.clone())?;
    let header = block.header;
    chain.import_block(block)?;

    let remaining: Vec<SignedTransaction> = pool.ordered_transactions().into_iter().cloned().collect();
    mempool_store::write(&mempool_file, &remaining)?;

    println!(
        "produced block {} ({} tx) -> {}",
        header.height,
        ready.len(),
        hex(&header.block_hash)
    );
    if dropped > 0 {
        println!("  dropped {dropped} pending transaction(s) that failed pool admission");
    }
    if !remaining.is_empty() {
        println!("  {} transaction(s) still pending for a future block", remaining.len());
    }
    Ok(())
}

fn run_import_block(data_dir: &Path, block_path: &Path) -> Result<(), CommandError> {
    ensure_initialized(data_dir)?;

    let bytes = std::fs::read(block_path)?;
    let (block, record_len) = record::decode_record(&bytes, 0)?;
    if record_len != bytes.len() {
        return Err(CommandError::TrailingBytesInBlockFile {
            path: block_path.to_path_buf(),
            extra: bytes.len() - record_len,
        });
    }

    let mut chain = load_chain(data_dir)?;
    let header = block.header;
    chain.import_block(block)?;

    println!("imported block {} -> {}", header.height, hex(&header.block_hash));
    Ok(())
}

fn run_replay(data_dir: &Path) -> Result<(), CommandError> {
    ensure_initialized(data_dir)?;

    let mut store = open_store(data_dir)?;
    store.recover()?;
    let blocks = store.load_blocks()?;
    let block_count = blocks.len();
    let result = execution::replay_chain(demo_genesis(), &blocks)?;

    println!("replayed {block_count} block(s) from genesis");
    println!("  final state root: {}", hex(&result.final_state_root));
    match (result.height, result.tip_hash) {
        (Some(height), Some(hash)) => println!("  tip: height {height} -> {}", hex(&hash)),
        _ => println!("  tip: none (still at genesis)"),
    }
    Ok(())
}

fn run_tip(data_dir: &Path) -> Result<(), CommandError> {
    ensure_initialized(data_dir)?;
    let chain = load_chain(data_dir)?;

    match chain.state().tip() {
        Some(header) => println!("tip: height {} -> {}", header.height, hex(&header.block_hash)),
        None => println!("tip: none (still at genesis)"),
    }
    Ok(())
}

fn run_accounts(data_dir: &Path) -> Result<(), CommandError> {
    ensure_initialized(data_dir)?;
    let chain = load_chain(data_dir)?;

    for name in DEMO_ACCOUNT_NAMES.iter().chain(std::iter::once(&FEE_COLLECTOR_NAME)) {
        let id = account_id_for_name(name);
        match chain.state().get_account(&id) {
            Some(account) => println!("{name:<14} balance {:<12} nonce {}", account.balance, account.nonce),
            None => println!("{name:<14} (not found in genesis)"),
        }
    }
    Ok(())
}

fn run_validate_storage(data_dir: &Path) -> Result<(), CommandError> {
    ensure_initialized(data_dir)?;
    let mut store = open_store(data_dir)?;
    let report = store.recover()?;

    println!("valid records: {}", report.valid_records);
    println!("original length: {} bytes", report.original_len);
    println!("final length: {} bytes", report.final_len);
    if report.truncated() {
        println!("truncated {} corrupt/partial byte(s) from the tail", report.truncated_bytes);
        if let Some(reason) = &report.rejected_reason {
            println!("  reason: {reason}");
        }
    } else {
        println!("no corruption found");
    }
    match (report.last_valid_height, report.last_valid_hash) {
        (Some(height), Some(hash)) => println!("last valid record: height {height} -> {}", hex(&hash)),
        _ => println!("last valid record: none"),
    }
    Ok(())
}

/// A minimal, crash-unsafe flat file of pending transactions -- deliberately
/// simpler than `storage::record`'s framing, since a pending pool is
/// ephemeral by definition (docs/storage.md's durability guarantees only
/// ever applied to committed blocks, never to the mempool). Each record is
/// the same fixed size, so the file is just those records concatenated,
/// with no header or length prefix needed.
mod mempool_store {
    use std::io::Write;
    use std::path::Path;

    use primitives::{Amount, Nonce};
    use transaction::transaction::{SignedTransaction, UnsignedTransaction};

    use super::CommandError;

    const RECORD_LEN: usize = 32 + 32 + 8 + 8 + 32 + 64;

    fn encode(tx: &SignedTransaction) -> [u8; RECORD_LEN] {
        let mut buf = [0u8; RECORD_LEN];
        let t = &tx.transaction;
        buf[0..32].copy_from_slice(&t.from);
        buf[32..64].copy_from_slice(&t.to);
        buf[64..72].copy_from_slice(&t.amount.to_le_bytes());
        buf[72..80].copy_from_slice(&t.nonce.to_le_bytes());
        buf[80..112].copy_from_slice(&tx.public_key);
        buf[112..176].copy_from_slice(&tx.signature);
        buf
    }

    fn decode(bytes: &[u8]) -> SignedTransaction {
        let mut from = [0u8; 32];
        from.copy_from_slice(&bytes[0..32]);
        let mut to = [0u8; 32];
        to.copy_from_slice(&bytes[32..64]);
        let amount = Amount::from_le_bytes(bytes[64..72].try_into().unwrap());
        let nonce = Nonce::from_le_bytes(bytes[72..80].try_into().unwrap());
        let mut public_key = [0u8; 32];
        public_key.copy_from_slice(&bytes[80..112]);
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&bytes[112..176]);

        SignedTransaction {
            transaction: UnsignedTransaction { from, to, amount, nonce },
            public_key,
            signature,
        }
    }

    pub fn read(path: &Path) -> Result<Vec<SignedTransaction>, CommandError> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let bytes = std::fs::read(path)?;
        if bytes.len() % RECORD_LEN != 0 {
            return Err(CommandError::CorruptMempoolFile {
                len: bytes.len(),
                record_len: RECORD_LEN,
            });
        }
        Ok(bytes.chunks_exact(RECORD_LEN).map(decode).collect())
    }

    pub fn write(path: &Path, txs: &[SignedTransaction]) -> Result<(), CommandError> {
        let mut buf = Vec::with_capacity(txs.len() * RECORD_LEN);
        for tx in txs {
            buf.extend_from_slice(&encode(tx));
        }
        std::fs::write(path, buf)?;
        Ok(())
    }

    pub fn append(path: &Path, tx: &SignedTransaction) -> Result<(), CommandError> {
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(&encode(tx))?;
        Ok(())
    }
}
