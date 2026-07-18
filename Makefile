.PHONY: fmt clippy check-default test check dist-check doc bench-release bench-profile bench-profile-matrix bench-partial-decompression

RELEASE_BENCH_QUERY_REPETITIONS ?= 500
RELEASE_BENCH_QUERY_WARMUP_REPETITIONS ?= 20
QZT_RELEASE_BENCH_QUERY_REPETITIONS ?= $(RELEASE_BENCH_QUERY_REPETITIONS)
QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS ?= $(RELEASE_BENCH_QUERY_WARMUP_REPETITIONS)
QZT_PARTIAL_BENCH_CORPUS_BYTES ?= 1073741824

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Compile the crate exactly as a default-features consumer sees it. The crate
# root denies missing public documentation for this curated surface.
check-default:
	cargo check --lib --bins

test:
	cargo test --all-targets --all-features

check: fmt clippy check-default test

# cargo-dist currently generates workflow-wide write permission. The wrapper
# reapplies QZT's least-privilege policy and verifies the checked-in result.
dist-check:
	./scripts/generate-release-workflow.sh --check

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

bench-release:
	cargo test --release --all-features --test release_hardening -- --nocapture

bench-profile:
	QZT_RELEASE_BENCH_QUERY_REPETITIONS=$(QZT_RELEASE_BENCH_QUERY_REPETITIONS) \
	QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=$(QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS) \
	cargo test --release --all-features --test release_hardening release_benchmark_profile -- --ignored --exact --nocapture

bench-profile-matrix:
	QZT_RELEASE_BENCH_QUERY_REPETITIONS=$(QZT_RELEASE_BENCH_QUERY_REPETITIONS) \
	QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=$(QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS) \
	cargo test --release --all-features --test release_hardening release_benchmark_profile_matrix -- --ignored --exact --nocapture

# This opt-in production probe is intentionally excluded from `make check`:
# generating and packing 1 GiB is evidence work, not a per-commit unit test.
bench-partial-decompression:
	QZT_PARTIAL_BENCH_CORPUS_BYTES=$(QZT_PARTIAL_BENCH_CORPUS_BYTES) \
	cargo test --release --all-features --test phase46_partial_decompression partial_decompression_probe_records_bounded_work_on_a_scalable_corpus -- --exact --nocapture
