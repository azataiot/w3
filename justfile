default:
    @just --list

build:
    cargo build --workspace

test:
    cargo test --workspace

lint:
    cargo clippy --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --all

qa:
    cargo fmt --all --check
    just lint
    just test

release level:
    cargo release {{level}} --execute

release-dry level:
    cargo release {{level}}
