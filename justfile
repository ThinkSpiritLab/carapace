set dotenv-load := true

test:
    cargo test -- --test-threads=1 --nocapture

dev:
    cargo fmt
    cargo check
    cargo clippy
