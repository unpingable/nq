# DNS wire-response fixtures — lab-backed compatibility evidence

`*.hex` are **real DNS response datagrams** captured from **BIND 9.18.49** (Debian bookworm,
Docker), one per `ResponseKind` that an RCODE+answer-section actually distinguishes. They are
the bytes `crates/nq-monitor/src/probe.rs::parse_response` (the hand-rolled UDP wire decoder)
is validated against — captured from a real authoritative resolver, not synthesized.

| file | query | resolver answer | response_kind |
|---|---|---|---|
| `success_a.hex` | `host.lab.test` A (id 0x1234) | RCODE 0, 1 answer (A 10.1.2.3) | success |
| `nodata_aaaa.hex` | `host.lab.test` AAAA (id 0x2345) | RCODE 0, 0 answers, SOA in authority (name exists, type doesn't) | nodata |
| `nxdomain_a.hex` | `missing.lab.test` A (id 0x3456) | RCODE 3 | nxdomain |
| `servfail_a.hex` | `x.dead.test` A (id 0x7890) | RCODE 2 (forward to dead upstream 192.0.2.1) | servfail |
| `refused_a.hex` | `other.test` A (id 0x4567) | RCODE 5 (out-of-zone, recursion off) | refused |
| `ptr.hex` | `3.2.1.10.in-addr.arpa` PTR (id 0x5678) | RCODE 0, 1 answer (PTR host.lab.test) | success (PTR is its own testimony) |
| `txt.hex` | `host.lab.test` TXT (id 0x6789) | RCODE 0, 1 answer (TXT) | success |

Each fixture is the raw response hex; the query id is the first two bytes and is the
`expected_id` the test passes to `parse_response`.

The five RCODE-bearing `response_kind`s (success / nodata / nxdomain / servfail / refused) are
now **all real-resolver-validated**. PTR + TXT validate answer decoding for non-A/AAAA types
(PTR especially: separate testimony, never an inferred reverse mapping). `timeout` /
`transport_error` are socket-layer outcomes (no answer to decode — covered by the synthetic
`outcome_from_wire` + `parse_response` defensive suites); `validation_failure` is reserved for a
future DNSSEC-validating probe (not V0).

## What this evidence is — and is not

> **Lab-backed compatibility evidence.** It testifies that NQ's DNS wire decoder correctly
> classifies real resolver responses (the success-vs-nodata split, NXDOMAIN, REFUSED) under
> declared lab conditions.
>
> It is **NOT** live-estate testimony. No receipt derived from these bytes says anything about
> any real network's DNS state. Synthetic compatibility ≠ live testimony.

`timeout` / `transport_error` are socket-layer outcomes (no answer to parse) covered by
`outcome_from_wire` tests; `validation_failure` is reserved for a future DNSSEC-validating probe
(not V0).

## Reproduce

```sh
docker run --rm -i debian:bookworm-slim bash -s    # then, inside:
#   apt-get update && apt-get install -y bind9 python3
#   named -g -c <conf: zone lab.test (host A 10.1.2.3), recursion no> &
#   python3: send a UDP DNS query to 127.0.0.1:53, print response.hex()
```
