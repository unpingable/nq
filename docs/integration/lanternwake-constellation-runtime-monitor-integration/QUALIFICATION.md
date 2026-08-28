# Lanternwake qualification

Campaign: **Lanternwake** / `constellation-runtime-monitor-integration`

## Deterministic source qualification

| Case | Evidence | Result |
|---|---|---|
| exact expected wire host | pure plus local HTTP fixture | accepted; canonical configured source retained |
| alternate reported host | pure substitution fixture plus local HTTP fixture | source error; no collector rows imported |
| expected host omitted | compatibility fixture | warning-only behavior retained |
| blank expected host | strict config fixture | validation refusal |
| missing/unsupported envelope schema | existing pull fixtures | source refusal |
| inaccessible source, timeout, malformed response | existing pull/source test and installation lanes | collection error/timeout, never target absent |
| stale source/host/service | existing detector/read-model tests | historical/stale remains distinct from current |
| partial collectors | existing collector/generation tests | partial coverage remains explicit |

Executed on 2026-08-28:

```text
cargo test -q -p nq-core config::tests
11 passed; 0 failed

cargo test -q -p nq-monitor pull::tests
8 passed; 0 failed

cargo test --workspace --lib
PASS across all workspace library targets

scripts/check-constellation-boundaries.sh
PASS for default and all-feature dependency resolutions
```

The second command emitted six pre-existing dead-code warnings outside this change. `cargo fmt --all -- --check` is not a clean baseline gate at source base `2e956d2`: it reports broad pre-existing formatting drift across unrelated files. Lanternwake did not rewrite those files.

The separate `nq-db` `operator_docs_contract` integration test ran five of six cases successfully; `canonical_deploy_configs_deserialize` failed because canonical `main` does not contain the fixture path `deploy/aggregator.json` that the test attempts to read. The failure predates and is unrelated to Lanternwake. The actual shipped config CLI tests passed: six `nq-monitor` cases and six `nq-witness` cases.

Kubernetes-specific permission/RBAC/API-failure/stale-cache/namespace-substitution fixtures are not claimed because no Kubernetes sensor was introduced. Those cases are mandatory before any future direct API primitive can land.

## Live read-only observations

At `2026-08-28T21:38:21.482907679Z`, `GET https://nq.neutral.zone/api/overview` returned generation `229949`, age 51 seconds, one host, 14 services, two SQLite databases, status `complete`, and two warnings. A subsequent documented-view query observed source `labelwatch-host` at `ok`, zero generations behind, and a current host row. A `v_metrics` query observed one attributed blackbox family including `probe_success=1` and `probe_http_status_code=200` for the public NQ endpoint.

This is live Linode-side host/runtime and HTTP-reachability testimony only. It is not TURNSTILE deployment qualification and does not establish universal health. The active k3s work reports no cluster/context/namespace/service account/workload/volume/image, so no k3s target was available to observe. The local qualification VM was powered off at its recorded closeout.
