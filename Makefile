# Aquachain agent-gateway local gates

CLIPPY_FLAGS := -D warnings
CARGO ?= cargo

.PHONY: help fmt format lint test coverage coverage-summary coverage-html tarpaulin \
	machete outdated fuzz fuzz-build geiger audit deny doc ci run build

.DEFAULT_GOAL := help

help:
	@echo "Aquachain agent-gateway targets"
	@echo "  make lint / test / doc / coverage / audit / deny / ci"

fmt:
	$(CARGO) fmt --check

format:
	$(CARGO) fmt

lint: fmt
	$(CARGO) clippy --all-targets -- $(CLIPPY_FLAGS)

test:
	$(CARGO) test

doc:
	RUSTDOCFLAGS='-D warnings' $(CARGO) doc --no-deps

coverage:
	mkdir -p coverage
	RUSTUP_TOOLCHAIN=stable $(CARGO) llvm-cov --locked --lcov --output-path coverage/lcov.info

coverage-summary:
	RUSTUP_TOOLCHAIN=stable $(CARGO) llvm-cov --locked --summary-only

coverage-html:
	mkdir -p coverage
	RUSTUP_TOOLCHAIN=stable $(CARGO) llvm-cov --locked --html --output-dir coverage/html

tarpaulin:
	mkdir -p coverage/tarpaulin
	$(CARGO) tarpaulin --locked --out Html --out Xml --output-dir coverage/tarpaulin

machete:
	$(CARGO) machete

outdated:
	$(CARGO) outdated --workspace

FUZZ_TARGET ?=
FUZZ_TIME ?= 10

fuzz-build:
	@test -d fuzz || (echo "no fuzz/; skip or add a cargo-fuzz workspace"; exit 1)
	cargo +nightly fuzz build

fuzz:
	@test -n "$(FUZZ_TARGET)" || (echo "set FUZZ_TARGET=…"; exit 1)
	cargo +nightly fuzz run $(FUZZ_TARGET) -- -max_total_time=$(FUZZ_TIME)

geiger:
	$(CARGO) geiger || true

audit:
	$(CARGO) audit

deny:
	$(CARGO) deny check

run:
	$(CARGO) run

build:
	$(CARGO) build --release

ci: lint test doc audit deny
