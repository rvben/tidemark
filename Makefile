.PHONY: build test lint fmt fmt-check clippy score check ci install release-patch release-minor release-major

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

release-patch:
	vership bump patch

release-minor:
	vership bump minor

release-major:
	vership bump major
