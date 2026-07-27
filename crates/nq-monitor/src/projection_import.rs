//! Shared persistence seam for receiver-owned projection receipts.
//!
//! Source-profile modules construct the receipt from the exact bytes and
//! packet/refusal they observed. This module only validates and publishes the
//! immutable artifact beneath the caller-provided packet store.

use nq_core::ProjectionReceipt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const RECEIPT_STORE_DIR: &str = ".projection-receipts";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct ProjectionReceiptStoreError {
    pub detail: String,
}

impl std::fmt::Display for ProjectionReceiptStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "projection receipt store error: {}", self.detail)
    }
}

impl std::error::Error for ProjectionReceiptStoreError {}

fn store_error(detail: impl Into<String>) -> ProjectionReceiptStoreError {
    ProjectionReceiptStoreError {
        detail: detail.into(),
    }
}

#[derive(Debug)]
pub struct ReceiptedImport<O, R> {
    pub outcome: Result<O, R>,
    pub receipt: ProjectionReceipt,
    pub receipt_path: PathBuf,
}

fn read_existing(
    path: &Path,
    expected_id: &str,
) -> Result<ProjectionReceipt, ProjectionReceiptStoreError> {
    let bytes = std::fs::read(path)
        .map_err(|e| store_error(format!("reading existing {}: {e}", path.display())))?;
    let receipt: ProjectionReceipt = serde_json::from_slice(&bytes).map_err(|e| {
        store_error(format!(
            "existing {} is not a strict receipt: {e}",
            path.display()
        ))
    })?;
    receipt
        .validate()
        .map_err(|e| store_error(format!("existing {} is invalid: {e}", path.display())))?;
    if receipt.receipt_id != expected_id {
        return Err(store_error(format!(
            "existing {} has receipt_id {:?}, expected {:?}",
            path.display(),
            receipt.receipt_id,
            expected_id
        )));
    }
    Ok(receipt)
}

/// Validate and publish a receipt without ever overwriting an existing
/// artifact. A repeated identity returns the first immutable receipt (and
/// therefore its original `imported_at`).
pub fn persist_projection_receipt(
    receipt: ProjectionReceipt,
    store: &Path,
) -> Result<(ProjectionReceipt, PathBuf), ProjectionReceiptStoreError> {
    receipt
        .validate()
        .map_err(|e| store_error(format!("refusing invalid receipt: {e}")))?;
    let receipt_hex = receipt
        .receipt_id
        .strip_prefix("sha256:")
        .ok_or_else(|| store_error("validated receipt_id lost sha256 prefix"))?;
    let receipt_dir = store.join(RECEIPT_STORE_DIR);
    let final_path = receipt_dir.join(format!("{receipt_hex}.projection-receipt.json"));

    if final_path.exists() {
        let existing = read_existing(&final_path, &receipt.receipt_id)?;
        return Ok((existing, final_path));
    }

    std::fs::create_dir_all(&receipt_dir)
        .map_err(|e| store_error(format!("creating {}: {e}", receipt_dir.display())))?;
    if final_path.exists() {
        let existing = read_existing(&final_path, &receipt.receipt_id)?;
        return Ok((existing, final_path));
    }

    let rendered = serde_json::to_vec_pretty(&receipt)
        .map_err(|e| store_error(format!("serializing projection receipt: {e}")))?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let tmp_path = receipt_dir.join(format!(
        ".{receipt_hex}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)
        .map_err(|e| store_error(format!("creating {}: {e}", tmp_path.display())))?;
    if let Err(e) = file.write_all(&rendered).and_then(|_| file.sync_all()) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(store_error(format!("writing {}: {e}", tmp_path.display())));
    }
    drop(file);

    match std::fs::hard_link(&tmp_path, &final_path) {
        Ok(()) => {
            let _ = std::fs::remove_file(&tmp_path);
            Ok((receipt, final_path))
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(&tmp_path);
            let existing = read_existing(&final_path, &receipt.receipt_id)?;
            Ok((existing, final_path))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(store_error(format!(
                "publishing immutable {}: {e}",
                final_path.display()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_core::{
        ProjectionMappingProfile, ProjectionReceiptMapping, ProjectionReceiptReplay,
        ProjectionReceiptSource, ProjectionSourceSystem, PROJECTION_RECEIPT_DOES_NOT_ESTABLISH,
        PROJECTION_RECEIPT_ESTABLISHES, PROJECTION_RECEIPT_SCHEMA,
    };

    fn receipt(imported_at: &str) -> ProjectionReceipt {
        let mut receipt = ProjectionReceipt {
            schema: PROJECTION_RECEIPT_SCHEMA.to_string(),
            receipt_id: String::new(),
            source: ProjectionReceiptSource {
                system: ProjectionSourceSystem::Continuity,
                schema: None,
                snapshot_identity: None,
                raw_digest: format!("sha256:{}", "1".repeat(64)),
                core_digest: None,
                record_ref: None,
            },
            mapping: ProjectionReceiptMapping {
                profile: ProjectionMappingProfile::ContinuityRecord,
                profile_version: format!("sha256:{}", "2".repeat(64)),
            },
            custody_basis: "external_projection".to_string(),
            packet: None,
            premises_as_coverage: vec![],
            projection_limits: vec![],
            replay: ProjectionReceiptReplay {
                outcome: "refused:malformed".to_string(),
                substitution: None,
            },
            contradiction_status: None,
            imported_at: imported_at.to_string(),
            establishes: PROJECTION_RECEIPT_ESTABLISHES.to_string(),
            does_not_establish: PROJECTION_RECEIPT_DOES_NOT_ESTABLISH
                .iter()
                .map(|s| s.to_string())
                .collect(),
        };
        receipt.seal().unwrap();
        receipt
    }

    #[test]
    fn repeated_identity_keeps_first_immutable_bytes() {
        let store = tempfile::tempdir().unwrap();
        let first = receipt("2026-07-26T22:10:00Z");
        let (stored, path) = persist_projection_receipt(first, store.path()).unwrap();
        let original = std::fs::read(&path).unwrap();

        let later = receipt("2026-07-27T01:00:00Z");
        assert_eq!(stored.receipt_id, later.receipt_id);
        let (stored_again, same_path) = persist_projection_receipt(later, store.path()).unwrap();
        assert_eq!(path, same_path);
        assert_eq!(stored_again.imported_at, "2026-07-26T22:10:00Z");
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}
