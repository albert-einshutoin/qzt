# サーバーログを保全し、必要範囲だけ提示する

**所要時間:** 15 minutes（約15分）  
**前提:** `qzt 0.1.0-pre.2`、`jq`。Linux/systemdと`minisign`は任意です。
安定したコマンド契約は[docs/CLI.ja.md](../CLI.ja.md)を参照してください。

この手順が作るのはtamper-evidentな技術記録です。固定後にbyteが変わったかは
検証できますが、ログ事象の真実性を確定するものでも法的助言でもありません。

## 1. テキストストリームをpackする

まず決定的なsampleで試します。

```sh
printf '%s\n' \
  '2026-07-19T01:00:00Z INFO service=api request_id=req-001 status=200' \
  '2026-07-19T01:01:00Z WARN service=api request_id=req-002 retry=1' \
  '2026-07-19T01:02:00Z ERROR service=api request_id=req-003 incident=INC-4242 status=503' \
  '2026-07-19T01:03:00Z INFO service=api request_id=req-004 status=200' \
  | qzt pack - -o daily.qzt
```

運用ではhostに合う一方を選び、収集権限とretention policyを別途reviewします。Bashまたは
Zshで実行してください。`pipefail`と`.partial` renameにより、上流collector失敗時に
validに見えるpartial archiveを公開しません。

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

`pack -`はCore streaming profileとfile outputを必要とします。QZTは区切りや
timestampを追加しないため、固定されるbyteは入力streamそのものです。

## 2. deep verifyし、決定的attestationを外部へanchorする

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

sampleでは次の安定fieldを返します（byte数は入力により変わります）。

```json
{"ok":true,"level":"deep","checked_chunks":1,"decoded_bytes":288}
```

containerとattestationは別のfailure domainへ保存します。署名とRFC 3161は
QZT外部のtrust operationです。[attestation署名guide](attestation.md)でminisign、
whole-file digest、timestamp検証まで確認してください。自己checksumと未署名attestationは
偶発破損を検出しますが、両方を置換できる攻撃者は再生成できます。敵対的改ざんへの耐性には、
攻撃者のcontrol外にあるprotected baseline、署名、trusted timestampが必要です。

## 3. Linuxで定期verifyする

完全なtemplateは
[`examples/qzt-verify.service`](examples/qzt-verify.service)、
[`examples/qzt-verify.timer`](examples/qzt-verify.timer)、
[`examples/qzt-verify-alert@.service`](examples/qzt-verify-alert@.service)にもあります。
serviceは明示した1 fileだけを検証します。複数archiveはservice instanceまたは
policy管理下のwrapperで列挙してください。

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

最小権限の`qzt-verify`/`qzt-alert` accountと、root所有で非writableなalert wrapperを
作ってから有効化します。wrapperは想定したunit name形式だけを受け入れ、messageへ安全に
encodeしてください。

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now qzt-verify.timer
systemctl list-timers qzt-verify.timer
```

deploy前にarchive sizeの実測から`MemoryMax`と`TimeoutStartSec`を調整します。oneshotが
activeな間はtimerが同じunitを再起動しないため、無制限のverifyは後続scheduleを落とします。

corrupt/unreadable inputはexit `1`になり`OnFailure`が作動します。exit `2`も設定不備
としてunitを失敗させます。JSON messageの文字列matchではなくunit resultを監視します。

## 4. 検索して、提示対象のrangeだけ復元する

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

検証sampleは`INC-4242`を返しました。hitは
`source=verified_original_bytes`ですが、hit intervalと物理decode境界は別です。
`decoded_bytes`は検証したcandidate byte、`physical_decoded_bytes`は物理的に展開した
完全chunkの量として観測します。

## Limitations（制約）

- QZIはderived/untrustedです。QZTをauthorityとして保持し、拒否されたsidecarは再構築します。
- tokenの複数語検索はco-occurrenceでphrase検索ではありません。rankingや正規化もありません。
- `capped`と`incomplete_reason`を確認せず、zero hitを否定結果として扱わないでください。
- 部分提示でも範囲に重なる完全chunkを展開します。返したbyteだけをdecodeする保証ではありません。
- 外部署名、timestamp、収集統制、retention policyはoperatorの責任です。
- QZTは圧縮しますが暗号化・redactionはしません。policyが許す段階でsecret/PIIを除去し、
  非共有の専用directory、`umask 077`、最小権限ACL、dataに合うstorage/transport
  encryptionを使います。

全コマンドはrelease binaryで実行済みです。
[tutorial validation record](tutorial-validation.md)を参照してください。
