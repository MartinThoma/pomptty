# Put the rustup toolchain first on PATH (the system cargo is too old — see README).
export PATH := $(HOME)/.cargo/bin:$(PATH)

CARGO ?= cargo

.PHONY: build run release test lint fmt clean

## Compile a debug build
build:
	$(CARGO) build

## Build and run a debug build
run:
	$(CARGO) run

## Compile an optimized build
release:
	$(CARGO) build --release

## Run the test suite
test:
	$(CARGO) test

## Check formatting and run clippy
lint:
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets -- -D warnings

## Format the source
fmt:
	$(CARGO) fmt

## Remove build artifacts
clean:
	$(CARGO) clean
