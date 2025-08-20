# Justfile for historee - Browser history analyzer

# Set shell for Windows PowerShell
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# Default target
default:
    @just --list

# Install historee as a CLI program
install:
    cargo install --path .

# Build release version
build:
    cargo build --release

# Run the program with default settings
run:
    cargo run --release

# Run with verbose logging
run-verbose:
    cargo run --release -- --verbose --top 10

# Run all checks (check, clippy, tests)
check-all: check clippy test

# Check code without building
check:
    cargo check

# Run clippy for linting
clippy:
    cargo clippy -- -D warnings

# Run tests
test:
    cargo test

# Initialize default patterns file
init-patterns:
    cargo run --release -- --init
