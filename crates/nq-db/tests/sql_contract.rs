//! SQL contract drift detection.
//!
//! Asserts that every view named in `docs/operator/sql-contract.md` as
//! part of the public SQL contract exists in the migrated database.
//!
//! Fails on public-contract drift only: a missing public view breaks the
//! test; an extra undocumented view does not. Internal/derived views are
//! allowed to come and go without notice per the contract.
//!
//! If a public view is intentionally removed, update both this list and
//! `docs/operator/sql-contract.md`, and add a `FEATURE_HISTORY.md` entry
//! per the contract's "Adding to the contract" section.
//!
//! If a new view should be public, add it here AND to sql-contract.md.

use nq_db::{migrate, open_ro, open_rw};

/// Views the operator SQL contract promises will exist.
///
/// Must match the public-tier listings in `docs/operator/sql-contract.md`.
/// Domain-specific public views (SMART, ZFS) are listed too — they exist
/// in the schema regardless of whether their collector emits rows.
const PUBLIC_CONTRACT_VIEWS: &[&str] = &[
    // Public contract views
    "v_hosts",
    "v_services",
    "v_sqlite_dbs",
    "v_sources",
    "v_metrics",
    // Public, evolving
    "v_warnings",
    "v_host_state",
    "v_admissibility",
    // Public, domain-specific
    "v_smart_witness",
    "v_smart_devices",
    "v_zfs_witness",
    "v_zfs_pools",
];

#[test]
fn public_contract_views_exist_after_migration() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("contract.db");

    {
        let mut wdb = open_rw(&db_path).unwrap();
        migrate(&mut wdb).unwrap();
    }

    let rdb = open_ro(&db_path).unwrap();
    let mut stmt = rdb
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type='view' ORDER BY name")
        .unwrap();
    let actual: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    let missing: Vec<&&str> = PUBLIC_CONTRACT_VIEWS
        .iter()
        .filter(|v| !actual.iter().any(|a| a == *v))
        .collect();

    assert!(
        missing.is_empty(),
        "SQL contract drift: public views missing from migrated schema: {:?}.\n\
         Actual views present: {:?}.\n\
         Either restore the view, or remove it from PUBLIC_CONTRACT_VIEWS \
         AND docs/operator/sql-contract.md AND add a FEATURE_HISTORY entry.",
        missing,
        actual,
    );
}
