# Lanternwake successor handoff

Campaign: **Lanternwake** / `constellation-runtime-monitor-integration`

Final classification: **MONITOR-INTEGRATION-LIVE-WITH-BOUNDED-BLIND-SPOTS**

## Status

Lanternwake was implemented and qualified against Classic NQ.
This does **not** make Classic NQ canonical for future development.

### Classic NQ

- repository/path: `/data/git/nq-root/nq`
- campaign worktree: `/data/git/.worktrees/nq-lanternwake-monitor-integration`
- branch: `campaign/lanternwake-constellation-runtime-monitor-integration`
- implementation-basis HEAD before this documentation-only freeze:
  `32bf75a750f47440df0c9ff29bbb85a5884c4760`
- status: operational compatibility/monitor implementation; Lanternwake branch
  frozen after closeout documentation
- role: legacy/current operational implementation basis for Lanternwake only

### Canonical successor

- repository/path: `/data/git/skunkworks/nq-ng`
- canonical branch: `main`
- HEAD: `59abd3bcb2d0cc30657a659b3ebc57b981289d9f`
- status: selected successor development line and default basis for future NQ
  development; not yet the operational authority and not cut over
- role: default basis for future NQ development

The repository's primary checkout was observed on the independent
`campaign/passive-watcher-succession-v1` line at
`675e247e85d8e2e1f2801c06445bf863f82b3a5b`. It was not modified. Future work
must start from a clean successor-NQ worktree at the appropriate canonical
line, not by taking custody of that active campaign checkout.

Current Cartography identifies NQ-ng as the selected successor while retaining
Classic as operational authority until an explicit switch. NQ-ng's governing
sequencing separately requires successor-native development from `main` and
states that Classic is not the source architecture. The operator ruling for
this closeout makes that selected successor the unqualified future-development
default without asserting an operational cutover.

## Semantic delta worth preserving

The reusable requirement is behavior, not a Classic-NQ file-level port:

```text
optional expected_reported_host
    +
exact observed publisher/reported-host identity
    ↓
equality required before collector-row import
    ↓
mismatch => source error / fail closed
    ↓
no imported observation attributed to the wrong expected host
```

This prevents valid-looking testimony from the wrong source being accepted as
testimony about the configured target.

- **Input facts:** the private source configuration's optional expected
  reported-host identity, the configured stable source identity and route, and
  the host identity carried by the decoded versioned publisher envelope.
- **Comparison point:** after the publisher envelope schema and body decode,
  before construction or import of any collector-derived rows.
- **Refusal behavior:** unequal identities produce a source error with bounded
  expected/observed identity detail. Collection failure remains distinct from
  target absence or a negative observation.
- **Rows not imported:** host, service/process/Docker, SQLite/WAL, log, metric,
  ZFS/SMART, GPU, and any other rows derived from that refused publisher
  response.
- **Configuration semantics:** the expected identity is optional for Classic
  compatibility, exact when supplied, and deployment-private. The configured
  source name remains its stable database identity. Omission does not prove a
  source binding.
- **Qualification cases:** exact identity acceptance, alternate-identity
  substitution refusal, omitted-value compatibility, blank-value config
  refusal, and matching/mismatching local HTTP fixtures. The mismatch fixture
  proves refusal before row import.
- **Receipt implications:** retain source, receive/collection times, and the
  refusal detail as source testimony; do not mint collector observations from
  the refused response. Downstream currentness and coverage must therefore be
  able to show source error/unknown/stale rather than silently treating the
  target as absent or current.

The successor may have a different architecture. It should implement this
semantic requirement in its native owner only if and when a forcing case
exists. This document does not prescribe files, crates, configuration layout,
or a cherry-pick.

## Reusable findings requiring revalidation

Every item below is **CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY
MUST BE RE-DERIVED**.

| Capability | Handoff status |
|---|---|
| host | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| systemd/service | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| Docker/PID | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| SQLite/WAL | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| logs | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| Prometheus | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| blackbox | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| root storage | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| ZFS/SMART | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| GPU | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| freshness | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |
| coverage | CLASSIC-NQ OBSERVED CAPABILITY; SUCCESSOR-NQ CAPABILITY MUST BE RE-DERIVED |

No successor parity is asserted by this list.

## Live evidence

The bounded Linode specimen recorded during Lanternwake observed a current
Classic-NQ source and host row, inventory counts of 14 service rows and two
SQLite rows, and an attributed blackbox metric family. The campaign receipt at
`2026-08-28T21:38:21.482907679Z` recorded generation `229949`, age 51 seconds,
and `complete` generation status. A final read-only wrap verification observed
generation `229985`, age 40 seconds, and `complete` status.

This is historical campaign evidence about those exact reads. It is not a
permanent liveness claim, a statement that every inventoried service/database
was individually checked, application success, deployment qualification, or
successor-NQ capability evidence.

## Remaining forcing trigger

Future work resumes only when a real deployment route or workload exists:

```text
real route appears
    ↓
use successor NQ's existing generic external observation if available
    ↓
determine the exact unanswered question
    ↓
only then consider minimum read-only Kubernetes facts
```

Do not start by porting Lanternwake. Direct Kubernetes observation remains
forcing-case gated; no client, RBAC, mutation, secret access, provider API, or
deployment-qualification coupling is authorized by this handoff.

## Future default

> For any future prompt that says "NQ" without qualification, resolve and use
> canonical successor NQ. Classic NQ requires explicit operator authorization.

This development default does not invalidate Classic history, Lanternwake
evidence, or Classic's current operational role.
