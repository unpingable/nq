-- service_state witness family (docs/working/decisions/preflights/SERVICE_STATE.md).
--
-- Native service-state observations: one row per (host, service_manager,
-- service_name) per generation. The WITNESS records the manager's NATIVE state
-- verbatim (active_state / sub_state / load_state / unit_file_state); the
-- EVALUATOR interprets it into claim verdicts.
--
-- There are deliberately NO `recovered` / `recovered_at` / `healthy` / `safe` /
-- `coverage` / `desired_state` / `expected_state` columns. Those are refused at
-- the claim layer (the evaluator's cannot_testify), never stored as observation
-- — storing them would launder interpretation into testimony. active does not
-- imply healthy; inactive does not imply broken; a missing row is "no witness",
-- not "false".

CREATE TABLE service_observations (
    observation_id    INTEGER PRIMARY KEY,
    generation_id     INTEGER NOT NULL REFERENCES generations(generation_id) ON DELETE CASCADE,
    host              TEXT NOT NULL,
    service_manager   TEXT NOT NULL
        CHECK (service_manager IN ('systemd', 'docker', 'process', 'unknown')),
    service_name      TEXT NOT NULL,
    active_state      TEXT NOT NULL,     -- native manager state verbatim
    sub_state         TEXT,              -- native sub-state, NULL when unavailable
    load_state        TEXT,              -- systemd load state (loaded/not-found/masked), NULL otherwise
    unit_file_state   TEXT,              -- enabled/disabled/masked/..., NULL when unavailable
    observed_at       TEXT NOT NULL      -- RFC3339 UTC
);

-- One current observation per service per generation. This is what makes the
-- writer's idempotent-same / explicit-conflict semantics enforceable and the
-- latest-per-tuple read well-defined.
CREATE UNIQUE INDEX idx_service_observations_identity
    ON service_observations (generation_id, host, service_manager, service_name);

CREATE INDEX idx_service_observations_tuple
    ON service_observations (host, service_manager, service_name, observed_at);
