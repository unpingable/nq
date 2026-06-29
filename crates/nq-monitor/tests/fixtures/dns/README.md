# DNS wire-response fixtures — lab-backed compatibility evidence

`*.hex` are **real DNS response datagrams** captured from **BIND 9.18.49** (Debian bookworm,
Docker), one per `ResponseKind` that an RCODE+answer-section actually distinguishes. They are
the bytes `crates/nq-monitor/src/probe.rs::parse_response` (the hand-rolled UDP wire decoder)
is validated against — captured from a real authoritative resolver, not synthesized.

| file | query | resolver answer |
|---|---|---|
| `success_a.hex` | `host.lab.test` A (id 0x1234) | RCODE 0, 1 answer (A 10.1.2.3) |
| `nodata_aaaa.hex` | `host.lab.test` AAAA (id 0x2345) | RCODE 0, 0 answers, SOA in authority (name exists, type doesn't) |
| `nxdomain_a.hex` | `missing.lab.test` A (id 0x3456) | RCODE 3 |
| `refused_a.hex` | `other.test` A (id 0x4567) | RCODE 5 (out-of-zone, recursion off) |

Each fixture is the raw response hex; the query id is the first two bytes and is the
`expected_id` the test passes to `parse_response`.

## What this evidence is — and is not

> **Lab-backed compatibility evidence.** It testifies that NQ's DNS wire decoder correctly
> classifies real resolver responses (the success-vs-nodata split, NXDOMAIN, REFUSED) under
> declared lab conditions.
>
> It is **NOT** live-estate testimony. No receipt derived from these bytes says anything about
> any real network's DNS state. Synthetic compatibility ≠ live testimony.

`servfail` (RCODE 2) shares the negative-RCODE decode path with `refused`; `timeout` /
`transport_error` are socket-layer outcomes (no answer to parse) covered by `outcome_from_wire`
tests; `validation_failure` is reserved for a future DNSSEC-validating probe (not V0).

## Reproduce

```sh
docker run --rm -i debian:bookworm-slim bash -s    # then, inside:
#   apt-get update && apt-get install -y bind9 python3
#   named -g -c <conf: zone lab.test (host A 10.1.2.3), recursion no> &
#   python3: send a UDP DNS query to 127.0.0.1:53, print response.hex()
```
