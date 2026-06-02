.PHONY: build test lint fmt fmt-check clippy score check ci install

build:
	cargo build --release

test:
	cargo nextest run
	cargo test --doc

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

clippy:
	cargo clippy --all-targets -- -D warnings

lint: fmt-check clippy

score: build
	clispec score ./target/release/tidemark

check: lint test

ci: check score

install:
	cargo install --path .
