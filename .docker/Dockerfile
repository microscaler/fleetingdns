FROM rustlang/rust:nightly-slim AS builder

RUN apt-get update && apt-get install -y \
  pkg-config \
  libssl-dev \
  build-essential \
  libclang-dev \
  curl \
  ca-certificates

RUN rustup component add rustfmt clippy
RUN cargo install cargo-audit

WORKDIR /usr/src/app
COPY . .

# Run checks and tests as part of the build
RUN cargo fmt --all -- --check
RUN cargo clippy --all-targets --all-features -- -D warnings
RUN cargo check
RUN cargo test --all

RUN cargo build --release

