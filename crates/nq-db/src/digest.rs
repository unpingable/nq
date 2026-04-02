//! Generation digest: content-addressed summary of what was seen and found.
//!
//! After publish + detect + lifecycle, we hash the generation's observable
//! state into a single hex string. If the findings set changes between
//! generations, the hash changes. Cheap drift detection.
//!
//! Uses FNV-1a (64-bit) — not cryptographic, but fast and sufficient for
//! detecting changes. This is not a security boundary.

use crate::WriteDb;

/// Compute and store a generation digest.
///
/// The digest covers:
/// - source_runs: who reported, who failed
/// - active warning_state: host/domain/kind/subject/severity
///
/// Written to generations.summary_hash.
pub fn seal_generation(db: &mut WriteDb, generation_id: i64) -> anyhow::Result<String> {
    let mut hasher = Fnv1a64::new();

    // Hash source runs for this generation
    {
        let mut stmt = db.conn.prepare(
            "SELECT source, status FROM source_runs WHERE generation_id = ?1 ORDER BY source",
        )?;
        let rows = stmt.query_map([generation_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (source, status) = row?;
            hasher.write(source.as_bytes());
            hasher.write(b":");
            hasher.write(status.as_bytes());
            hasher.write(b"\n");
        }
    }

    hasher.write(b"---\n");

    // Hash active warnings
    {
        let mut stmt = db.conn.prepare(
            "SELECT host, domain, kind, subject, severity FROM warning_state ORDER BY host, kind, subject",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        for row in rows {
            let (host, domain, kind, subject, severity) = row?;
            hasher.write(host.as_bytes());
            hasher.write(b"/");
            hasher.write(domain.as_bytes());
            hasher.write(b"/");
            hasher.write(kind.as_bytes());
            hasher.write(b"/");
            hasher.write(subject.as_bytes());
            hasher.write(b"=");
            hasher.write(severity.as_bytes());
            hasher.write(b"\n");
        }
    }

    let hash = format!("{:016x}", hasher.finish());

    db.conn.execute(
        "UPDATE generations SET summary_hash = ?1 WHERE generation_id = ?2",
        rusqlite::params![&hash, generation_id],
    )?;

    Ok(hash)
}

/// FNV-1a 64-bit hasher. No dependencies needed.
struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001B3;

    fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}
