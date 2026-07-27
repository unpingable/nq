# nq-check-pack-labelwatch

This optional pack makes the existing Labelwatch deployment assembly explicit.
It owns stable descriptors and strict, private-value-free configuration for
Labelwatch service, SQLite, log, and metric targets.

There was no coherent Labelwatch collector to move from NQ. The pack therefore
produces a typed collection plan for the monitor's reusable collection
primitives; it deliberately does not claim to be an independently executable
collector. A composition root must supply those primitives before enabling the
pack.

Service targets name their acquisition adapter (`systemd`, `docker`, or
`pid_file`) and native target explicitly. The pack never infers systemd from a
missing manager field.

Compilation never enables Labelwatch. No hostname, filesystem path, service
name, URL, or threshold has a Labelwatch-specific default.
