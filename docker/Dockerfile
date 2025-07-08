FROM rust:1.77-slim AS builder
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev build-essential libclang-dev ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release --bins --workspace

FROM debian:bookworm-slim AS runtime
RUN groupadd -r app && useradd --no-log-init -r -g app app
WORKDIR /app
COPY --from=builder /src/target/release/*-bin /app/
USER app
ENTRYPOINT ["/app/dnsd-bin"]
CMD ["--addr", "0.0.0.0:5353"]
