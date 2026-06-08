# QZT Portable Vectors

Vectors are stored as lowercase hexadecimal `.qzt.hex` files so they remain
reviewable in git. A third-party runner should decode the hex to bytes, then
apply `manifest.tsv`.

Regenerate the deterministic vectors with:

```sh
cargo test --test phase22_vectors -- --ignored regenerate_vectors
```

The default vector runner verifies that the committed files match deterministic
generation and that valid/corrupt expectations are honored.
