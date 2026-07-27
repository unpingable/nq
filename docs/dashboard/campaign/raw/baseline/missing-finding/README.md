# Baseline missing-finding reproduction

This directory is raw campaign evidence. Do not rewrite the operator
transcripts into a cleaner narrative.

## Environment

- Commit: `ba5a79d50d95901625fae1edf7e9145871f51f44`
- Captured: 2026-07-26
- Route: `GET /finding/error_shift/labelwatch/logwatch`
- HTTP status: `200`
- Database: disposable copy migrated to schema 64
- Browser: Google Chrome 140, headless, 1440 × 1200
- Screenshot SHA-256:
  `38ea8aae476116f030bd9791016c0c3560b59ea1374bfef990bed65376870074`

## Reproduction

The requested `(kind, host, subject)` tuple did not exist in
`warning_state`. The production route rendered:

- `Finding not found`;
- an error-rate-spike headline and detector-specific explanatory copy;
- `0 consecutive generations · since ?`;
- all six lifecycle controls;
- detector-specific pivots and raw SQL.

The route used the real migrated database, production router, and real browser
renderer. The absent finding itself is a deterministic fixture condition; it
is not evidence from a deployed NQ instance.

## Raw artifacts

- `page.png`: browser screenshot
- `operator-production-sre.md`: fresh Codex production-SRE read
- `operator-sleep-deprived.md`: fresh Codex fatigue read

