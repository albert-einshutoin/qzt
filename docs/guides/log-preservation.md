# Preserve and selectively disclose server logs

**Time:** 15 minutes  
**Prerequisites:** `qzt 0.1.0-pre.2`; `jq`; optional Linux/systemd and
`minisign`. The stable command contract is [docs/CLI.md](../CLI.md).

This workflow creates tamper-evident technical records. It can show whether
stored bytes changed after fixation; it does not establish that a log event was
true or provide legal advice.

## 1. Pack a text stream

Use this deterministic sample first:

```sh
printf '%s\n' \
  '2026-07-19T01:00:00Z INFO service=api request_id=req-001 status=200' \
  '2026-07-19T01:01:00Z WARN service=api request_id=req-002 retry=1' \
  '2026-07-19T01:02:00Z ERROR service=api request_id=req-003 incident=INC-4242 status=503' \
  '2026-07-19T01:03:00Z INFO service=api request_id=req-004 status=200' \
  | qzt pack - -o daily.qzt
```

The same stdin path accepts an operational stream. Choose one command for the
host; review collection permissions and retention policy separately. Run these
blocks in Bash or Zsh: `pipefail` plus the `.partial` rename prevents an upstream
collector failure from publishing a valid-looking partial archive.

```sh
# Linux
set -euo pipefail
umask 077
archive="logs-$(date +%F).qzt"
partial="${archive}.partial"
test ! -e "$archive" && test ! -e "$partial"
trap 'rm -f -- "$partial"' EXIT
journalctl --since yesterday | qzt pack - -o "$partial"
mv -- "$partial" "$archive"
trap - EXIT

# macOS
set -euo pipefail
umask 077
archive="logs-$(date +%F).qzt"
partial="${archive}.partial"
test ! -e "$archive" && test ! -e "$partial"
trap 'rm -f -- "$partial"' EXIT
log show --last 1d --style ndjson | qzt pack - -o "$partial"
mv -- "$partial" "$archive"
trap - EXIT
```

`pack -` requires the Core streaming profile and a file output. QZT never
inserts separators or timestamps, so the fixed bytes are exactly the input
stream.

## 2. Deep-verify and anchor a deterministic attestation

```sh
set -euo pipefail
umask 077
qzt verify daily.qzt --deep --format json
attestation=daily.attest.json
partial="${attestation}.partial"
test ! -e "$attestation" && test ! -e "$partial"
trap 'rm -f -- "$partial"' EXIT
qzt attest daily.qzt > "$partial"
jq -e '
  .verify.level == "deep" and
  .verify.decoded_bytes == .original_size and
  .verify.checked_chunks == .chunk_count
' "$partial"
mv -- "$partial" "$attestation"
trap - EXIT
```

The sample verification emits the following stable fields (byte counts vary
with input):

```json
{"ok":true,"level":"deep","checked_chunks":1,"decoded_bytes":288}
```

Store the container and attestation in separate failure domains. Signing and
RFC 3161 anchoring are external trust operations; follow the
[attestation signing guide](attestation.md) for minisign, whole-file digests,
and timestamp verification. A self-checksum plus an unsigned attestation detects
accidental corruption, but an attacker who can replace both can regenerate
them. Adversarial tamper evidence requires a protected baseline, signature, or
trusted timestamp outside that attacker's control.

## 3. Schedule verification on Linux

Install the complete templates from
[`examples/qzt-verify.service`](examples/qzt-verify.service),
[`examples/qzt-verify.timer`](examples/qzt-verify.timer), and
[`examples/qzt-verify-alert@.service`](examples/qzt-verify-alert@.service).
The service intentionally uses an explicit file path; generate one service per
retained object or call a policy-owned wrapper that enumerates the archive.

```ini
[Unit]
Description=Deep-verify one QZT archive
OnFailure=qzt-verify-alert@%n.service
RequiresMountsFor=/archive

[Service]
Type=oneshot
User=qzt-verify
ExecStart=/usr/local/bin/qzt verify /archive/daily.qzt --deep --format json
TimeoutStartSec=30min
MemoryMax=1G
TasksMax=32
NoNewPrivileges=true
CapabilityBoundingSet=
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
PrivateNetwork=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictSUIDSGID=true
LockPersonality=true
StandardOutput=journal
StandardError=journal
```

```ini
[Unit]
Description=Run QZT archive verification daily

[Timer]
OnCalendar=daily
RandomizedDelaySec=5min
Persistent=true
Unit=qzt-verify.service

[Install]
WantedBy=timers.target
```

```ini
[Unit]
Description=Alert after failed QZT verification (%i)

[Service]
Type=oneshot
User=qzt-alert
ExecStart=/usr/local/sbin/qzt-verify-alert %i
TimeoutStartSec=2min
MemoryMax=256M
TasksMax=16
NoNewPrivileges=true
CapabilityBoundingSet=
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictSUIDSGID=true
LockPersonality=true
```

Enable the timer after creating least-privileged `qzt-verify` and `qzt-alert`
accounts and a root-owned, non-writable alert wrapper. The wrapper must accept
only the expected unit-name shape and safely encode it before sending a message:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now qzt-verify.timer
systemctl list-timers qzt-verify.timer
```

Tune `MemoryMax` and `TimeoutStartSec` from measured archive size before
deployment. A timer does not start a second instance while this oneshot remains
active, so an unlimited verifier can silently skip later schedules.

`qzt verify` exits `1` for corrupt/unreadable input, which activates
`OnFailure`. Exit `2` is a configuration/usage defect and also fails the unit.
Treat the JSON as untrusted log data and alert on the unit result, not on
string-matching its error message.

## 4. Search, then restore only the disclosed range

Build a rebuildable token sidecar, search in JSON mode, and feed the hit's
half-open byte interval to `range`:

```sh
set -euo pipefail
qzt sidecar-rebuild daily.qzt --index token -o daily.qzi
qzt search daily.qzt INC-4242 --sidecar daily.qzi --format json > hit.json
jq '{hit: .hits[0], metrics: .metrics, capped, incomplete_reason}' hit.json

jq -e '
  .capped == false and
  .incomplete_reason == null and
  (.hits | length) > 0 and
  (.hits[0].logical_offset | type) == "number" and
  .hits[0].logical_offset >= 0 and
  (.hits[0].byte_length | type) == "number" and
  .hits[0].byte_length > 0
' hit.json

read offset length <<EOF
$(jq -r '.hits[0] | "\(.logical_offset) \(.byte_length)"' hit.json)
EOF
qzt range daily.qzt --bytes "${offset}:$((offset + length))"
```

The validated sample returned `INC-4242`. A hit has
`source=verified_original_bytes`; the search verifies candidate bytes before
reporting it. The returned hit interval is not the physical decode boundary.
Inspect `decoded_bytes` and `physical_decoded_bytes` to see candidate bytes
verified and complete chunks physically decompressed.

## Limitations

- QZI is derived and untrusted; keep the QZT container as the authority and
  rebuild a rejected sidecar.
- Token search is co-occurrence, not phrase search, and has no normalization or
  ranking. Check `capped` and `incomplete_reason` before treating no hits as a
  negative result.
- Partial disclosure still decompresses every chunk overlapping the requested
  range. It does not imply that only returned bytes were decoded.
- External signatures, timestamps, acquisition controls, and retention policy
  remain operator responsibilities.
- QZT compresses but does not encrypt or redact logs. Remove secrets/PII before
  fixation where policy permits, write into a dedicated non-shared directory,
  use `umask 077` and least-privilege ACLs, and protect archives with
  storage/transport encryption appropriate to the data.

The commands above were executed against the release binary. See the
[tutorial validation record](tutorial-validation.md).
