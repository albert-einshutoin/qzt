# Signing and anchoring QZT attestations

`qzt attest` verifies a container and emits a small, deterministic JSON document
that can be signed, timestamped, or recorded in an external ledger. Keep the
attestation and its signature in storage separate from the QZT container so a
single loss or compromise does not remove both the evidence and its external
proof.

The default verification level is `deep`. The output is one canonical JSON line:
keys are in lexicographic order, strings contain no insignificant whitespace,
hexadecimal values are lowercase, and the file ends with exactly one newline.
It deliberately excludes the source path, host name, and current time.

## 1. Generate an attestation and fingerprint

```sh
qzt attest evidence.qzt > evidence.attest.json
sha256sum evidence.attest.json
```

On macOS, where `sha256sum` is not installed by default, use:

```sh
shasum -a 256 evidence.attest.json
```

Use `--level quick` or `--level normal` only when the reduced verification is an
intentional operational trade-off. The selected level and measured coverage are
recorded in the `verify` object, so a verifier can reject a weaker claim.

Verification levels are not interchangeable:

| Level | Payload coverage | Appropriate use |
| --- | --- | --- |
| `quick` | Parses and validates container structure; does not read chunk payloads | Triage only |
| `normal` | Adds compressed-chunk checksums and the container checksum when present; does not reconstruct original bytes | Storage scrubbing where full decode is intentionally deferred |
| `deep` | Adds chunk decode, uncompressed checksums, and the original-content checksum | External evidence, signing, and timestamping |

`checked_chunks` is the number of chunk records covered by the selected
verification path; it does not mean that `quick` decoded those chunks. A relying
party accepting an external evidence claim should parse the JSON and require all
of the following before checking its signature:

```sh
jq -e '
  .verify.level == "deep" and
  .verify.decoded_bytes == .original_size and
  .verify.checked_chunks == .chunk_count
' evidence.attest.json
```

If policy requires the exact QZT container bytes—not only the reconstructed
original content—also require `.container_checksum != null`:

```sh
jq -e '.container_checksum != null' evidence.attest.json
```

Install `jq` with the operating system package manager when enforcing these
examples in shell. Production verifiers should apply the equivalent checks in
their JSON parser before trusting a signature.

## 2. Sign with minisign

Install minisign using your operating system's package manager or follow the
[official installation instructions](https://jedisct1.github.io/minisign/).
Generate a key pair once, protect the secret key, then sign the attestation:

```sh
minisign -G
minisign -Sm evidence.attest.json \
  -t "archived $(date -u +%Y-%m-%dT%H:%M:%SZ)"
```

Distribute `minisign.pub` through a trusted channel. Later, verify the detached
signature (`evidence.attest.json.minisig`) with:

```sh
minisign -Vm evidence.attest.json -p minisign.pub
```

The timestamp above is a signed operator assertion in minisign's trusted
comment. It is useful context, but it is not an independently trusted timestamp.

## 3. Request an RFC 3161 timestamp

OpenSSL's `ts` command creates and verifies RFC 3161 timestamp messages. See the
[official OpenSSL documentation](https://docs.openssl.org/master/man1/openssl-ts/)
for the full trust-store and policy options.

Create a DER-encoded request that includes a nonce and asks the Time Stamping
Authority (TSA) to include its certificate:

```sh
TSA_POLICY_OID='1.2.3.4.5' # Replace with the policy OID published by your TSA.
openssl ts -query -data evidence.attest.json -sha256 -cert \
  -tspolicy "$TSA_POLICY_OID" \
  -out evidence.attest.tsq
```

Sending is intentionally separate from OpenSSL's `ts` command. Set `TSA_URL` to
the HTTPS endpoint supplied by your chosen TSA, review its certificate policy,
and retain the complete response:

```sh
curl --fail --silent --show-error \
  -H 'Content-Type: application/timestamp-query' \
  -H 'Accept: application/timestamp-reply' \
  --data-binary @evidence.attest.tsq \
  "$TSA_URL" \
  -o evidence.attest.tsr
```

Verify the response against both the original request and the TSA trust chain.
Use CA and intermediate certificate files published by that TSA:

```sh
openssl ts -verify \
  -queryfile evidence.attest.tsq \
  -in evidence.attest.tsr \
  -CAfile tsa-root.pem \
  -untrusted tsa-intermediates.pem
```

Do not treat a successful HTTP response as proof. Archive the `.tsr`, verify it,
and pin the intended TSA policy and trust roots in your operational procedure.
If your TSA requires its default policy instead, document that explicit choice
and inspect the returned token's policy OID rather than silently accepting any
default.

## 4. Verify the evidence later

Perform all three checks: verify the QZT container, reproduce its attestation,
then verify the external signature or timestamp.

```sh
qzt verify evidence.qzt --deep
qzt attest evidence.qzt | diff - evidence.attest.json
minisign -Vm evidence.attest.json -p minisign.pub

# If RFC 3161 was used instead of, or in addition to, minisign:
openssl ts -verify \
  -data evidence.attest.json \
  -in evidence.attest.tsr \
  -CAfile tsa-root.pem \
  -untrusted tsa-intermediates.pem
```

An empty `diff` confirms that the verified container still produces the exact
bytes that were signed. The signature or TSA response must be checked separately;
QZT does not perform those external trust operations.

## What each layer proves

- QZT's recorded level defines its coverage: `quick` checks parsed structure,
  `normal` additionally checks compressed chunks and the optional container
  checksum, and `deep` additionally decodes chunks and checks original bytes.
- The canonical attestation gives external tools stable bytes to sign or anchor.
- A minisign signature proves that the holder of the corresponding secret key
  signed those bytes. Its trusted comment is signed metadata, not third-party time.
- A correctly validated RFC 3161 response proves that a trusted TSA observed the
  attestation digest no later than the time in its signed response.

None of these layers proves that the original text was true when collected.
Preserve acquisition logs, key custody records, TSA responses, trust-chain
material, and retention policy alongside the evidence workflow.
