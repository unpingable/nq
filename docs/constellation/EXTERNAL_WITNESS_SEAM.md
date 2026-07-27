# External witness seam validation

Date: 2026-07-27 (America/New_York)

This report records a real public-boundary validation run. Generated producer
artifacts lived only under `/tmp`; no source or historical material in
`zab2nq` was modified.

## Boundary under test

Producer:

- repository: `/home/jbeck/git/nq-root/zab2nq`
- HEAD: `4a57c5ebcfe74ee93b0f73d190361bf155107cb2`
- worktree before and after: clean
- output family: `zab2nq_monitor_definition`

Consumer:

- `nq-witness-tool validate-set`
- public artifact API: `nq_witness::adopt_packet_set`
- accepted wire schema: `nq.witness.v1`
- set identity domain: `nq.witness_set.v1`

There is no source dependency from `nq-witness` to `zab2nq`. The producer was
not compiled into NQ, registered as a monitor check, or selected by default
configuration.

## Fresh reproduction

The documented conversion was run from the producer checkout with bytecode
writes disabled and with a fresh output directory:

```text
env PYTHONDONTWRITEBYTECODE=1 /usr/bin/time \
  -f 'elapsed_seconds=%e user_seconds=%U system_seconds=%S max_rss_kib=%M' \
  .venv/bin/python tools/monitor_to_witness.py corpus \
  --records-dir records \
  --config conversion/corpus-config.json \
  --output-dir /tmp/nq-zab2nq-seam.VG4H39QJ/packets \
  --report /tmp/nq-zab2nq-seam.VG4H39QJ/report.json \
  --manifest /tmp/nq-zab2nq-seam.VG4H39QJ/manifest.sha256
```

Producer result:

```text
source records considered: 6874
converted:                6874
refused:                  0
elapsed:                  171.50 seconds
user CPU:                 129.76 seconds
system CPU:               3.54 seconds
maximum RSS:              327940 KiB
packet directory bytes:   57852757
```

The fresh output was byte-identical to the producer's committed inventory:

```text
manifest SHA-256:
84dbe8f382d36e206d246fc67bf2a9cdd4a241cefee90cdb972604ae983aec3f

report SHA-256:
1ab80e85b097e342de11b425056c0b15720d0d1a3cbb4cd867ddeaccbc17e610
```

Both `cmp` checks against
`inventory/monitor_to_witness_manifest.sha256` and
`inventory/monitor_to_witness_report.json` returned success.

## Public-boundary validation

The complete fresh set was then passed through the standalone public
consumer:

```text
/usr/bin/time \
  -f 'elapsed_seconds=%e user_seconds=%U system_seconds=%S max_rss_kib=%M' \
  target/debug/nq-witness-tool validate-set \
  --directory /tmp/nq-zab2nq-seam.VG4H39QJ/packets \
  --manifest /tmp/nq-zab2nq-seam.VG4H39QJ/manifest.sha256
```

Relevant machine-readable result fields:

```json
{
  "schema": "nq.witness_tool.result.v1",
  "tool_version": "0.1.0",
  "operation": "validate_set",
  "status": "accepted",
  "packet_count": 6874,
  "witness_set_schema": "nq.witness_set.v1",
  "witness_set_digest": "sha256:f09c93fb2e29a48d0d0e50ab35326557bcc567f12578eb9f9b8399ee72a6de40",
  "manifest": {
    "digest": "sha256:84dbe8f382d36e206d246fc67bf2a9cdd4a241cefee90cdb972604ae983aec3f",
    "entry_count": 6874,
    "raw_byte_digests_verified": true,
    "directory_membership_verified": true
  },
  "custody_basis_counts": {
    "external_projection": 6874
  },
  "witness_type_counts": {
    "zab2nq_monitor_definition": 6874
  },
  "access_path_counts": {
    "archive_read": 6874
  },
  "position_counts": {
    "platform": 6874
  },
  "external_projection_packet_count": 6874,
  "native_custody_packet_count": 0,
  "runtime_occurrence_established_by_validation": false
}
```

Consumer timing:

```text
elapsed:     12.83 seconds
user CPU:    12.04 seconds
system CPU:  0.78 seconds
maximum RSS: 367120 KiB
```

An offline, locked install from the current source checkout also succeeded:

```text
cargo install --path crates/nq-witness \
  --bin nq-witness-tool \
  --root /tmp/nq-witness-tool-install.De4VOc77 \
  --offline --locked

release build and install: 6.76 seconds
```

The installed release binary accepted the same 6,874-packet set with the same
manifest and set identities in 2.21 seconds (2.01 seconds user CPU, 0.19
seconds system CPU, 366548 KiB maximum RSS). This proves the binary can be
installed and run from the source constellation checkout; it is not evidence
of a registry-based standalone install.

The attempted release-artifact preflight:

```text
cargo package -p nq-witness --allow-dirty --offline
```

failed because the configured offline crates.io index had no matching
`nq-protocol` package. Consequently, this run does not earn an independently
downloadable `nq-witness` registry release. Online clean-room verification,
publication ordering, and registry availability of the shared protocol leaf
remain installation-track work; source-path composition must not be
misreported as independent artifact installation.

The manifest byte digest and the witness-set digest differ by design. The
first binds the producer's filename/hash manifest bytes. The second is the
order-independent identity of the adopted JCS/SHA-256 packet identities.

## Producer checks

The producer's documented unit command completed in 0.61 seconds:

```text
Ran 13 tests
OK (skipped=1)
```

Twelve tests passed. Its optional real-consumer test was skipped because the
producer test constructed the undocumented, incorrect local path
`/home/jbeck/git/nq-root/nq-root/nq/target/debug/nq-monitor`. This is a
producer-side path leak. It was not repaired or hidden. The complete direct
validation above used the new independently named public witness tool and did
not rely on that path.

The producer report and `MONITOR_TO_WITNESS.md` still name
`nq-monitor validate-witness` as the authoritative validator. That is
historical monitor-boundary coupling in read-only producer documentation and
report metadata. A producer follow-on should point its optional consumer test
and future generated report metadata at the independently owned witness tool.
The packet envelopes themselves do not embed a validator binary or checkout
path.

The producer's historical integrity verifier completed in 114.11 seconds:

```text
records: 6874
schema-invalid records: 0
trigger dependencies: 1885; resolved_id populated: 1885
duplicate record ids: 0
VERIFY_RESULT: PASS
```

## Authority effect

This run establishes:

- every manifest entry names one regular packet file and every packet file is
  named by the manifest;
- every exact packet byte hash matches;
- all 6,874 envelopes are structurally valid `nq.witness.v1` artifacts;
- the packet set has the deterministic public identity shown above;
- all packets declare external-projection custody and archive-read access.

It does not establish:

- that any Zabbix trigger evaluated or fired;
- that a runtime subject, state, event, ordering, or cause existed;
- that source definitions are true or operationally sufficient;
- that the artifacts support any NQ claim;
- evidence sufficiency, freshness, authorization, or an NQ disposition.

The static external packet set crossed the witness artifact boundary. It did
not become a monitor observation or a default deployment dependency.
