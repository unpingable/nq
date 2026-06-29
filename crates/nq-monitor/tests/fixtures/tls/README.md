# TLS certificate fixtures — lab-backed compatibility evidence

`lab_leaf.pem` is a **controlled self-signed certificate** generated with OpenSSL 3.0
specifically to exercise NQ's TLS cert verdict ladder against real cert material:

```
openssl req -x509 -newkey rsa:2048 -nodes -keyout /dev/null -out lab_leaf.pem -days 365 \
  -subj "/CN=tls-lab.test/O=NQ TLS Lab" \
  -addext "subjectAltName=DNS:tls-lab.test,DNS:www.tls-lab.test"
```

Known facts (frozen with the blob):

- subject / issuer: `CN=tls-lab.test, O=NQ TLS Lab` (self-signed)
- SAN (DNS): `tls-lab.test`, `www.tls-lab.test` — **multi-SAN**, exercising what the
  single-SAN `nq.neutral.zone` leaf fixture (`tls_cert_transport.rs`) does not
- validity: `2026-06-29T16:31:29Z` → `2027-06-29T16:31:29Z`
- sha256 fingerprint: `06:3F:00:37:EA:50:15:C8:FC:34:84:50:3D:F2:2F:F1:D1:98:2F:0B:70:E6:73:02:8F:E2:25:50:54:58:EF:1D`

## What this evidence is — and is not

> **Lab-backed compatibility evidence.** It testifies that NQ's TLS cert parser
> (`parse_presented_cert`) and verdict core (`evaluate_tls_cert`) correctly extract
> fields from, and classify, a real certificate under declared lab conditions — across
> the verdict ladder (valid / within-warning-horizon / expired-under-probe-clock /
> name-mismatch) via the probe's injected clock.
>
> It is **NOT** live-estate testimony. The cert is a throwaway self-signed lab blob; no
> receipt derived from it says anything about any real endpoint's certificate state.

The probe's verdict ladder was previously exercised only by **synthetic** `PresentedCert`
values; this fixture drives it from a real parsed certificate. The cert is intentionally
self-signed — WebPKI chain validity is tested separately against the bundled roots; these
fixtures isolate the parse + name/expiry-vs-probe-clock verdicts.
