# nq-check-pack-host

The conservative host check pack performs cheap, local, read-only observation
of load, memory, root-filesystem capacity, uptime, kernel, and boot identity.
It is the only pack eligible for a minimal public default.

Collection is capability-honest: unsupported substrates and fields are typed
as such. The pack does not infer operational impact, cause, or an NQ
disposition.
