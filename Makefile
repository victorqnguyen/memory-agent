.PHONY: build build-wasm test lint install audit clean check

build:
	cargo build --release -p memory-agent

build-wasm:
	cargo build --release -p memory-wasm --target wasm32-wasip1

test:
	cargo test

lint:
	cargo clippy -- -D warnings

install:
	cargo install --path crates/memory-agent

audit:
	cargo audit

clean:
	cargo clean

check: lint test audit
