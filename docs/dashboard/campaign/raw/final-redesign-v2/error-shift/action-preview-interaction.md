# Suppress preview interaction record

- Fixture server: deterministic `dashboard_fixture` specimen
- Source state: subsequently committed as `9cd302a`
- Route: stable finding detail for `local/app-1/error_shift/labelwatch`
- Interaction: entered actor `synthetic-operator`, chose **Preview Suppress**
- API path exercised: production `/api/finding/action/preview`
- Server result: target and preconditions validated
- Previewed transition: `new → suppressed`
- Optional expiry: absent
- Confirmation: not selected
- Mutation performed: no
- Finding state after capture: `new`
- Underlying monitored system: synthetic fixture only; no system actuation path exists

The screenshot is evidence of the real preview path and rendered semantics. It is not evidence of a live deployment or a completed action.
