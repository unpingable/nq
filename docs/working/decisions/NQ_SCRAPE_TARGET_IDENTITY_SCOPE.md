# Scope packet — scrape-target as series identity (referred, operator-authorized)

**Status:** referred design packet. **NOT authorized. NOT built.** The loop reached an SCA stop condition (irreversibility + historical-interpretation policy choice) and referred this rather than executing it. The doctrine line: *additive evidence plumbing is authorized; identity-semantics migration is not. You may make provenance visible; you may not redefine what a series is without authorization.*
**Filed:** 2026-06-12 (ag-claude loop, batch A boundary).
**Companion (shipped):** migration 058 + `NQ_ECOSYSTEM_TRIAGE.md` § Re-survey. The additive half (provenance queryable + honest collision guard) is shipped; this is the identity half.

## What's already shipped (the additive half — migration 058)

- `scrape_target_name` / `scrape_target_url` columns on `series`, exposed via `v_metrics`.
- Provenance carried `MetricSample` → `MetricRow` → `series` (the wire→batch drop point is fixed).
- **Honest collision guard:** because identity is unchanged, one `series_id` can still receive samples from two scrape targets. Rather than last-write-wins (a queryable lie), a target mismatch marks the series ambiguous — `scrape_target_name`/`url` nulled, `scrape_target_collision = 1` (sticky). Three honest states: attributed / no-provenance / ambiguous.

So today: provenance is **visible and honest**, but two probes with identical `metric_name`+`labels` from different targets **collapse** (and are flagged ambiguous, not distinguished).

**Repair side-effect, classified (2026-06-12).** `publish_batch` section 7 (the series/metrics upsert) is now guarded by `if !batch.metric_sets.is_empty()`. This belongs to the **migration-058 forward-compat repair**, *not* to silence or any metrics policy — it was bundled into commit `d955578` only by timing. The guard exists because the unconditional `prepare_cached` of the series-upsert validated the 058 columns against the schema even with zero metric rows, which broke `publish_batch` on a pre-058 DB (the upgrade-path fixture). Empty `metric_sets` was already a no-op (the per-host delete+replace only runs for hosts present in `metric_sets`), so the guard is behavior-preserving — it is plumbing, not a decision. Naming it here so a future reader does not mistake it for a metrics-emission policy ("guards have a way of becoming policy wearing a hoodie").

## What this packet would build (the identity half)

Make scrape-target part of series identity so different targets get different `series_id`s and never collapse.

1. **Rebuild `series`** with `UNIQUE (metric_name, labels_json, scrape_target_name)` (or an explicit identity dimension — see open question 2).
2. **Coordinated rebuild of `metrics_current` + `metrics_history`** — they FK-reference `series(series_id)`. Under `foreign_keys = ON` (NQ's open path, `connect.rs:28`), dropping `series` triggers an implicit-DELETE FK violation from the children, so the rebuild must drop+recreate children first (the migration-010 dance, in reverse). This is the irreversible, high-blast-radius part.
3. **Retire the collision guard** — once identity distinguishes targets, `scrape_target_collision` can never be set; the column becomes vestigial (drop it, or keep as a tripwire).

## Why this is operator-gated (the three gates it hits)

- **Irreversibility** — a 3-table coordinated rebuild of the central metrics store on three production hosts. Not additive; not trivially reversible.
- **Identity-semantics change** — it redefines what a `series` *is*. That is a doctrine-level decision, not an implementation detail.
- **Historical interpretation** — existing rows have NULL provenance; the migration must decide what they *mean* (below). That is a policy choice the loop must not make.

## Decisions the operator owns (named, not pre-decided)

1. **Existing-row provenance.** Pre-058 / non-prometheus series have NULL scrape_target. On the identity rebuild, do they become: `NULL` (honest "unknown"), a synthetic `''`/`unknown` sentinel folded into identity, or does the migration **refuse** to run until they're classified? Recommendation to weigh: keep `NULL` and let the UNIQUE treat NULL as its own identity bucket — but SQLite treats NULLs as distinct in UNIQUE, which would *un-dedup* existing non-prometheus series. That alone may force a `''` sentinel for the identity column. This interaction is the crux and is exactly why it's referred.
2. **Identity shape.** Does `labels_json` stay the canonical identity and scrape-target become a third identity column, OR does scrape provenance get folded into a normalized identity tuple? (Affects every `series` join in `detect.rs` and `v_metrics`.)
3. **Same-batch multi-target.** Independent of cross-generation collision: two targets emitting identical `metric_name`+`labels` in **one** cycle currently produce two `MetricRow`s mapping to one `series_id` → a `metrics_current` PK violation `(host, series_id)` that errors the whole batch. The identity rebuild fixes this for free (distinct series_ids). Until then it's latent (one prometheus_target today). The operator should know the identity migration is *also* the fix for this, not just for cross-gen collision.

## Forcing condition

This becomes load-bearing the moment a **second bare-metric scrape target** lands (e.g. nq-blackbox promotes multiple Bucket-1/Bucket-2 probes emitting `probe_success` with no distinguishing label). At that point the collapse stops being theoretical and the collision guard starts firing on real data. Until then, the additive half is sufficient and honest.

## Exact operator decision needed

"Authorize the series-identity migration" + answers to decisions 1–2 above (decision 3 falls out of the rebuild). Until then the additive half stands and the collision guard keeps the partial state honest.
