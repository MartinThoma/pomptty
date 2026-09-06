# Put the rustup toolchain first on PATH (the system cargo is too old — see README).
export PATH := $(HOME)/.cargo/bin:$(PATH)

CARGO ?= cargo

.PHONY: build run release test lint fmt clean windows deb rpm man icons

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

## Cross-compile a release .exe for Windows from Linux. One-time setup: the
## mingw-w64 linker (`sudo apt install mingw-w64` on Debian/Ubuntu); the
## x86_64-pc-windows-gnu rustup target is added automatically if missing.
windows:
	@command -v x86_64-w64-mingw32-gcc >/dev/null || { \
		echo "error: mingw-w64 not found (needed to link the .exe)."; \
		echo "  install it first, e.g.: sudo apt install mingw-w64"; \
		exit 1; \
	}
	rustup target add x86_64-pc-windows-gnu
	$(CARGO) build --release --target x86_64-pc-windows-gnu
	@echo "Built target/x86_64-pc-windows-gnu/release/pomptty.exe"

## Build a .deb package. Installs cargo-deb if missing (no sudo needed).
deb:
	command -v cargo-deb >/dev/null || $(CARGO) install cargo-deb --locked
	$(CARGO) deb
	@echo "Built target/debian/*.deb"

## Build an .rpm package. Installs cargo-generate-rpm if missing (no sudo
## needed); strips debug symbols first, per its own recommendation.
rpm:
	command -v cargo-generate-rpm >/dev/null || $(CARGO) install cargo-generate-rpm --locked
	$(CARGO) build --release --locked
	strip -s target/release/pomptty
	$(CARGO) generate-rpm
	@echo "Built target/generate-rpm/*.rpm"

## Preview the man page.
man:
	man -l packaging/pomptty.1

## Re-rasterize packaging/pomptty.svg into the hicolor PNG sizes.
icons:
	@command -v rsvg-convert >/dev/null || { echo "error: rsvg-convert not found (librsvg2-bin)"; exit 1; }
	@for s in 16 32 48 64 128 256; do \
		mkdir -p packaging/icons/$${s}x$${s}; \
		rsvg-convert -w $$s -h $$s packaging/pomptty.svg -o packaging/icons/$${s}x$${s}/pomptty.png; \
	done
	@echo "Wrote packaging/icons/*/pomptty.png"
