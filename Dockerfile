########## 1. Builder Stage ##########
FROM rust:1-bookworm AS builder

WORKDIR /app

# Install libraries required by SQLx and XML/Crypto builds
RUN apt-get update && \
    apt-get install -y \
        pkg-config \
        libssl-dev \
        libpq-dev \
        ca-certificates \
        libxml2-dev && \
    rm -rf /var/lib/apt/lists/*

# Use stable (edition 2024 is already supported on stable)
RUN rustup default stable

# Install SQLx CLI
RUN cargo install --version="~0.7" sqlx-cli \
    --no-default-features \
    --features rustls,postgres

# Copy cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./

# Pre-build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy full project
COPY . .

# Build final binary
RUN cargo build --release


########## 2. Runtime Stage ##########
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies (keep all requested packages)
RUN apt-get update && \
    apt-get install -y \
        ca-certificates \
        libssl-dev \
        libpq5 \
        libxml2-dev \
        wget && \
    rm -rf /var/lib/apt/lists/*

# Copy compiled server binary
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Copy SQLx CLI (glibc-compatible)
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx

# Copy migrations
COPY --from=builder /app/migrations /app/migrations

ENV PORT=8080
EXPOSE 8080

CMD ["sh", "-c", "sqlx migrate run && ./stc-server"]
