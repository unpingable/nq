# Quickstart configuration artifacts

These files are the literal, unprivileged configurations shown in the
single-host installation documentation.

- `publisher.json` runs the compatibility `nq-witness` publisher on loopback
  with only the generic local host observation.
- `aggregator.json` pulls that publisher and keeps its SQLite database and
  liveness export in the current directory.
- `monitor-only.json` starts only the central monitor/dashboard with no
  sources. It proves that the runtime role is separable; it provides no host
  evidence and an empty issue list must not be interpreted as universal
  health.

Copy these files rather than retyping JSON. Configuration validation is
side-effect free and should be run before either process starts.

These are trial configurations, not production service-account or remote-host
settings. The production examples remain under `deploy/examples/`.
