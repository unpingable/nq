# Provenance

This project is human-directed and AI-assisted. Final design authority,
acceptance criteria, and editorial control rest with the human author.
AI contributions were material and are categorized below by function.

## Human authorship

The author defined the project direction, requirements, and design intent.
AI systems contributed proposals, drafts, implementation, and critique under
author supervision; they did not independently determine project goals or
deployment decisions. The author reviewed, revised, or rejected AI-generated
output throughout development.

## AI-assisted collaboration

### Architectural design, invariants, and failure-domain taxonomy

Lead collaboration: ChatGPT (OpenAI). Heavy involvement in failure-domain
classification, the cybernetic failure taxonomy (15 Δ-domains), roadmap
review, "what not to build" decisions, operator-vs-dashboard posture,
and the priority-vs-domain framing.

Also contributed: Gemini (Google) for independent validation of the
failure-domain model and architectural review. DeepSeek for storage
architecture (SQLite + Parquet + DuckDB tiering).

### Implementation, tests, and deployment

Lead collaboration: Claude (Anthropic) via Claude Code. Heavy contributions
to all source code, test suites, migrations, collector implementations,
detector logic, web UI, notification pipeline, CLI, docs, and deployment
automation. Assembled architectural decisions into working Rust code across
multiple intensive sessions.

### Cross-project design (Governor, WLP, Standing)

Claude Code (Agent Governor session) contributed crosswalk analysis between
NQ and Governor, identifying shared patterns (generation digests, escalation
as hysteresis, two-ledger model). ChatGPT contributed the nq-standing
design note and constitutional architecture framing.

Claude (claude.ai) contributed independent product-eye review, naming
("Nerd Queue"), launch positioning, and the "NQ accuses, nlai interprets,
Governor authorizes" framing.

### Validation and adversarial review

Gemini (Google) and DeepSeek were used as secondary validators. Gemini's
"your SQLite situation is becoming philosophical" observation confirmed the
failure-domain classification was legible to external reviewers.

## Provenance basis and limits

This document is a functional attribution record based on commit history,
co-author trailers (where present), project notes, and documented working
sessions. It is not a complete forensic account of all contributions.

Some AI contributions (especially design critique, rejected alternatives,
and footguns avoided) may not appear in repository artifacts or commit
metadata.

## What this document does not claim

- No exact proportional attribution. Contributions are categorized by
  function, not quantified by token count or lines of code.
- Design and implementation were not cleanly sequential. Architecture
  informed code, code revealed design gaps, and the feedback loop was
  continuous.
- "Footguns avoided" and "ideas that didn't ship" are real contributions
  that leave no artifact. This document cannot fully account for them.

---

This document reflects the project state as of 2026-04-02 and may be revised.
