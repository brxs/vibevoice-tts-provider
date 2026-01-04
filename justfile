# Default recipe - show available commands
default:
    @just --list

# Initialize submodules
init:
    git submodule update --init --recursive

# Build the project
build:
    cargo build

# Build release version
build-release:
    cargo build --release

# Run the server with TUI (default)
run *ARGS:
    cargo run -- {{ARGS}}

# Run the server in release mode with TUI
run-release *ARGS:
    cargo run --release -- {{ARGS}}

# Run the server in headless mode (no TUI, logs to console)
run-headless *ARGS:
    cargo run -- --no-tui {{ARGS}}

# Run headless in release mode
run-headless-release *ARGS:
    cargo run --release -- --no-tui {{ARGS}}

# Check code without building
check:
    cargo check

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Format check (CI)
fmt-check:
    cargo fmt -- --check

# Clean build artifacts
clean:
    cargo clean

# Regenerate proto files
proto:
    cargo build --build-plan > /dev/null

# Run all checks (format, lint, build)
ci: fmt-check lint build

# Watch for changes and rebuild
watch:
    cargo watch -x check

# Update dependencies
update:
    cargo update
