//! Day 5 Hour 3 -- fuzzes `storage::recovery::recover`, the crash-recovery
//! scan documented in "Recovery" in `docs/storage.md`. `recover` already
//! calls `record::decode_record` in a loop (fuzzed on its own by
//! `block_decoder`), stopping at the first invalid or partial record and
//! truncating the file to the last known good offset -- this target treats
//! arbitrary bytes as a whole log file on disk, the same shape `recover`
//! sees after a real crash (a partial write, a corrupted sector, a
//! byte-flipped file), and confirms the full scan-and-truncate algorithm
//! survives it.
//!
//! Fuzz rule (see `docs/fuzzing.md`):
//!   - arbitrary bytes may make `recover` return `Err` (e.g. an I/O error)
//!   - arbitrary bytes must never panic or produce undefined behavior
//!   - `recover` must not silently accept a malformed tail as valid: every
//!     record it counts must have actually round-tripped through
//!     `decode_record`, and the file must never end up longer than it
//!     started (a truncate-only operation can't grow the file)

#![no_main]

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use tempfile::TempDir;

use storage::recovery::recover;

/// One fuzzing-process-lifetime scratch directory, reused (and its single
/// log file overwritten) on every input -- avoids paying `tempdir()`'s
/// cost per iteration while still exercising the real filesystem
/// `recover` reads from.
fn scratch_path() -> &'static std::path::Path {
    static DIR: OnceLock<TempDir> = OnceLock::new();
    static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir = DIR.get_or_init(|| tempfile::tempdir().expect("tempdir"));
        dir.path().join("fuzz-recovery.log")
    })
}

fuzz_target!(|data: &[u8]| {
    let path = scratch_path();

    {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .expect("write fuzz input as log file");
        file.write_all(data).expect("write fuzz bytes");
        file.sync_data().expect("sync fuzz bytes");
    }

    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("reopen fuzz log for recovery");

    let Ok(report) = recover(path, &file) else {
        return;
    };

    // `recover` only ever truncates -- the file can't have grown.
    assert!(report.final_len <= report.original_len);
    assert_eq!(report.original_len, data.len() as u64);
    assert_eq!(report.truncated_bytes, report.original_len - report.final_len);

    // A record is only counted if it fully round-tripped through
    // `decode_record`, so `valid_records > 0` implies both a height and a
    // hash were recorded (and vice versa).
    assert_eq!(report.valid_records > 0, report.last_valid_height.is_some());
    assert_eq!(report.valid_records > 0, report.last_valid_hash.is_some());

    let on_disk = std::fs::metadata(path).expect("stat recovered log").len();
    assert_eq!(on_disk, report.final_len);
});
