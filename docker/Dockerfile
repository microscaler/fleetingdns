FROM rust:1.77-slim AS builder

RUN apt-get update && apt-get install -y \
  pkg-config \
  libssl-dev \
  build-essential \
  libclang-dev \
  curl \
  ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy

WORKDIR /usr/src/app
COPY . .
RUN cargo build --release -p dnsd-bin

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN groupadd -r app && useradd --no-log-init -r -g app app
WORKDIR /app
COPY --from=builder /usr/src/app/target/release/dnsd-bin /app/
USER app
ENTRYPOINT ["/app/dnsd-bin"]
CMD ["--addr", "0.0.0.0:5353"]
