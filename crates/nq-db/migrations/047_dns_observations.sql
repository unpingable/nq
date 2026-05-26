-- Migration 047: dns_observations substrate for the `dns_state` preflight
-- witness family (third bespoke claim kind, V0).
--
-- See docs/working/gaps/DNS_WITNESS_FAMILY_GAP.md.
--
-- Design discipline (constitutional from the gap):
--   - Substrate only. No probe, no evaluator, no HTTP, no registry.
--   - One row per (vantage_host, resolver, query_name, query_type)
--     observation. The evaluator (later slice) reads the latest row per
--     tuple.
--   - response_kind is a closed enum of what the resolver returned. The
--     enum slot for `validation_failure` is reserved for a future
--     DNSSEC-validating probe; V0 collectors never emit it.
--   - "No row exists for this tuple" is evaluator territory
--     (insufficient_coverage), not a substrate kind. Persisting a
--     sentinel for absence would launder absence into testimony.
--   - ON DELETE CASCADE on generation_id so retention-driven generation
--     pruning carries observations along.

CREATE TABLE dns_observations (
    observation_id    INTEGER PRIMARY KEY,
    generation_id     INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    vantage_host      TEXT NOT NULL,
    resolver          TEXT NOT NULL,
    query_name        TEXT NOT NULL,
    query_type        TEXT NOT NULL,
    response_kind     TEXT NOT NULL
        CHECK (response_kind IN (
            'success', 'nodata', 'nxdomain', 'servfail', 'refused',
            'timeout', 'transport_error', 'validation_failure'
        )),
    rcode             INTEGER,           -- raw DNS RCODE when applicable
    answer_summary    TEXT,              -- fingerprint of answer set; NULL for negative/error
    min_ttl_seconds   INTEGER,           -- min TTL across answer set; NULL when no answers
    duration_ms       INTEGER NOT NULL,
    observed_at       TEXT NOT NULL,     -- RFC3339 UTC, query-completion time
    error_detail      TEXT               -- short transport-error string; NULL otherwise
);

-- Latest-per-tuple lookup: the evaluator (later slice) reads
--   WHERE vantage_host = ? AND resolver = ? AND query_name = ? AND query_type = ?
--   ORDER BY observed_at DESC LIMIT 1
-- against this ordering. The leading tuple-fields support equality
-- narrowing; the trailing observed_at DESC supports the LIMIT 1 step.
CREATE INDEX idx_dns_observations_tuple_recent
    ON dns_observations(vantage_host, resolver, query_name, query_type, observed_at DESC);
