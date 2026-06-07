.PHONY: fmt clippy test check

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test --all-targets --all-features

check: fmt clippy test
