# Database Substrate Ladder

Status: candidate coverage framing
Authority: roadmap vocabulary only; not an implementation commitment

SQLite is NQ's compact substrate lab. It exposes persistence pressure, WAL behavior, freelist growth, lock contention, and witness/lifecycle semantics in a small observable package.

The MySQL family is the credibility bridge. MySQL, MariaDB, and Percona Server make the same admissibility problems legible to operators with production scars: lock waits, slow-query shifts, replication lag, buffer pressure, deadlocks, temp-table spill, connection pressure, and exporter silence.

Postgres is the portability proof. If the witness discipline survives a second major production database family, the model is not just SQLite-shaped or MySQL-shaped.

## Compatibility is not testimony

"MySQL-compatible" is not a witness contract.

A witness may share protocol access across MySQL, MariaDB, and Percona Server without being entitled to make the same claim across all three. Shared wire compatibility does not imply shared performance schema semantics, storage-engine behavior, replication semantics, exporter metric vocabulary, or operational interpretation.

Default shape:

- use `mysql_family_*` only where the testimony boundary survives across flavors;
- carry `engine_flavor`, `engine_version`, and `storage_engine` in witness packets where relevant;
- split into flavor-specific witnesses only when behavior diverges.

Keeper:

> Compatibility is not testimony.

## Non-goals

- no database support commitment;
- no MySQL-first product roadmap;
- no claim that PMM / Percona / exporters are replaced;
- no root-cause claims from database telemetry alone;
- no automatic ladder from SQLite checks to MySQL-family checks.

## Composition candidate

A future SQL-derived finding may compose database testimony with app and external-vantage testimony:

> Labelwatch backlog rose while MySQL-family lock waits increased and external probes degraded.

Can testify:

> these signals co-occurred under fresh dependencies.

Cannot testify:

> MySQL caused the incident.
