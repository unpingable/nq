# Host-trust boundary

**Status:** as-built security boundary.

> NQ trusts the host on which a local witness runs. Its structural integrity
> checks begin after collection; they do not authenticate the collector or
> defeat root compromise, kernel compromise, a malicious local operator, or an
> actor who can rewrite and reseal artifacts.

## What is inside the trusted base

- The host kernel, `/proc`, filesystem, service manager, sockets, and helper
  commands seen by `nq-witness` are assumed to report honestly enough for the
  deployment's purpose.
- The monitor database, configuration, and binaries are assumed not to be
  replaced by an actor with equivalent local privilege.
- The network path to a remote witness must be protected by deployment
  controls. The built-in HTTP service provides neither TLS nor client
  authentication.

An actor with root, kernel, or operator-equivalent access can fabricate source
observations, modify the database, replace binaries, or rewrite both an
artifact and its checksum. Those changes can propagate to findings, exports,
and receipts. NQ has no in-process mechanism that makes a compromised host
testify honestly.

## What the integrity fields establish

- A receipt `content_hash` checks whether its canonical body matches the
  checksum stored inside it.
- A `WitnessRef.digest` can identify the exact portable packet envelope that a
  receipt references when the packet is retained.
- Generation summary hashes provide change detection for their bounded stored
  content; they are not cryptographic host attestation.

These checks detect corruption and edits that were not resealed. They do not
identify who produced an artifact, prove that an observation was truthful, or
prevent an actor who controls the artifact from computing a new matching hash.

## Deployment consequences

- Run NQ under a dedicated unprivileged account and keep binaries and configs
  root-owned.
- Bind same-host services to loopback. For remote witnesses, use a private or
  VPN interface and firewall the endpoint to the monitor.
- Keep backups and long-lived receipt/packet artifacts in storage the NQ
  service account cannot rewrite.
- Treat Docker-socket access, privileged SMART/ZFS helpers, broad journal
  access, and writable application directories as explicit trust expansions.

If hostile-host assurance, non-repudiable operator identity, or multi-tenant
isolation is required, add controls outside the current NQ boundary: hardware-
or platform-backed identity, authenticated transport, signatures, and an
independently administered log or artifact store. A second host can provide a
different vantage and detect absence or disagreement, but it does not repair a
compromised first host's local testimony.

See [Production Deployment](../operator/deployment.md) for concrete service and
network guidance, [Scope and Witness Model](SCOPE_AND_WITNESS_MODEL.md) for
vantage semantics, and [Claim Custody](CLAIM_CUSTODY.md) for receipt limits.
