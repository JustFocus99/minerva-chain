//! Crash recovery: scan the log from the beginning, accept valid records,
//! stop at the first invalid or partial one, and truncate the file to the
//! last known good offset. See "Recovery" in `docs/storage.md` — this is a
//! direct implementation of that algorithm, not a looser variant of it.

use std::fs::File;
use std::path::Path;

use primitives::{BlockHash, BlockHeight};

use crate::error::StorageError;
use crate::record;

/// Summary of what a [`crate::append_log::AppendOnlyBlockStore::recover`]
/// run found and did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReport {
    /// Number of records accepted, from the start of the file.
    pub valid_records: usize,
    /// File length before recovery ran.
    pub original_len: u64,
    /// File length after recovery ran (== `original_len` if nothing was
    /// truncated).
    pub final_len: u64,
    /// `original_len - final_len`. Zero if the file needed no truncation.
    pub truncated_bytes: u64,
    /// Height of the last accepted record, if any were accepted.
    pub last_valid_height: Option<BlockHeight>,
    /// Block hash of the last accepted record, if any were accepted.
    pub last_valid_hash: Option<BlockHash>,
    /// Why the scan stopped before the end of the file, if it did.
    pub rejected_reason: Option<String>,
}

impl RecoveryReport {
    /// True if the file needed to be shortened to remove a partial or
    /// corrupted tail.
    pub fn truncated(&self) -> bool {
        self.truncated_bytes > 0
    }
}

/// Scans `path`'s contents from byte 0, accepting valid records and
/// stopping at the first invalid or partial one. Truncates `file` to the
/// end of the last accepted record and returns a report of what happened.
pub fn recover(path: &Path, file: &File) -> Result<RecoveryReport, StorageError> {
    tracing::info!("storage_recovery_started");

    let bytes = std::fs::read(path)?;
    let original_len = bytes.len() as u64;

    let mut offset = 0usize;
    let mut valid_records = 0usize;
    let mut last_valid_height = None;
    let mut last_valid_hash = None;
    let mut rejected_reason = None;

    while offset < bytes.len() {
        match record::decode_record(&bytes, offset) {
            Ok((block, record_len)) => {
                valid_records += 1;
                last_valid_height = Some(block.header.height);
                last_valid_hash = Some(block.header.block_hash);
                offset += record_len;
            }
            Err(err) => {
                rejected_reason = Some(err.to_string());
                break;
            }
        }
    }

    let final_len = offset as u64;
    if final_len != original_len {
        file.set_len(final_len)?;
    }

    tracing::info!(
        valid_records,
        truncated_bytes = original_len - final_len,
        height = ?last_valid_height,
        block_hash = ?last_valid_hash.map(|hash| primitives::to_hex(&hash)),
        error = ?rejected_reason,
        "storage_recovery_completed"
    );

    Ok(RecoveryReport {
        valid_records,
        original_len,
        final_len,
        truncated_bytes: original_len - final_len,
        last_valid_height,
        last_valid_hash,
        rejected_reason,
    })
}
