FROM rust:1 AS builder

WORKDIR /app

RUN mkdir -p migrations

RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN rm -rf src

COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/ultor ./ultor
COPY --from=builder /app/migrations ./migrations

RUN useradd -m appuser
USER appuser

CMD ["./ultor"]
