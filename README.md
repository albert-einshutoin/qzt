# QZT

QZT is a cold evidence container format. This repository contains the Rust reference implementation.

日本語版: [README.ja.md](README.ja.md)

## Local Quality Gate

```sh
make check
```

The gate runs:

```text
- cargo fmt --all -- --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo test --all-targets --all-features
```

## Phase Plan

Implementation proceeds through `tasks/Phase0.md` to `tasks/Phase13.md`.

Progress is tracked in `tasks/status.md`.

## Product Critique

An adversarial counterargument against the current product spec and phase plan is documented in [`docs/QZT_v0.1_Product_Counterargument.md`](docs/QZT_v0.1_Product_Counterargument.md).
