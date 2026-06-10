.PHONY: fmt clippy check-default test check bench-release

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Compile the crate exactly as a default-features consumer sees it.
# (missing_docs warnings are a known deferred backlog; this catches
# compile errors and new non-doc warnings in the curated surface.)
check-default:
	cargo check --lib --bins

test:
	cargo test --all-targets --all-features

check: fmt clippy check-default test

bench-release:
	cargo test --release --all-features --test release_hardening -- --nocapture
