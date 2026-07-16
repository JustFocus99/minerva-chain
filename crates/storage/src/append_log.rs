use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use block::block::Block;

use crate::error::StorageError;
use crate::record::{self, COMMIT_MARKER};
use crate::recovery::{self, RecoveryReport};

/// Durable storage for the canonical chain's blocks, in commit order.
pub trait BlockStore {
    /// Appends `block` as the next record in the log. Returns only after
    /// the record — including its commit marker — is durable on disk.
    fn append_block(&mut self, block: &Block) -> Result<(), StorageError>;

    /// Decodes and returns every record currently in the log, in order.
    /// Does not run recovery — a log with a corrupted or partial tail
    /// causes this to return an error; call [`BlockStore::recover`] first
    /// if the log may not be clean.
    fn load_blocks(&self) -> Result<Vec<Block>, StorageError>;

    /// Scans the log from the beginning, truncating away any partial or
    /// corrupted tail, and reports what it found. See `docs/storage.md`.
    fn recover(&mut self) -> Result<RecoveryReport, StorageError>;
}

/// A single append-only file on disk holding one record per block, in the
/// format defined by `docs/storage.md`.
pub struct AppendOnlyBlockStore {
    path: PathBuf,
    file: File,
}

impl AppendOnlyBlockStore {
    /// Opens (creating if necessary) the log file at `path` for appending.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self { path, file })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl BlockStore for AppendOnlyBlockStore {
    fn append_block(&mut self, block: &Block) -> Result<(), StorageError> {
        // Two-phase write: the body is durable before the commit marker is
        // written, so a crash between the two phases leaves a record that
        // recovery can identify as partial and discard, rather than one
        // that looks committed but isn't. See "The commit marker" in
        // docs/storage.md.
        let body = record::encode_record_without_marker(block)?;

        self.file.write_all(&body)?;
        self.file.flush()?;
        self.file.sync_data()?;

        self.file.write_all(&[COMMIT_MARKER])?;
        self.file.flush()?;
        self.file.sync_data()?;

        Ok(())
    }

    fn load_blocks(&self) -> Result<Vec<Block>, StorageError> {
        let bytes = std::fs::read(&self.path)?;

        let mut blocks = Vec::new();
        let mut offset = 0usize;
        while offset < bytes.len() {
            let (block, record_len) = record::decode_record(&bytes, offset)?;
            blocks.push(block);
            offset += record_len;
        }

        Ok(blocks)
    }

    fn recover(&mut self) -> Result<RecoveryReport, StorageError> {
        recovery::recover(&self.path, &self.file)
    }
}
