########## 1. Builder Stage ##########
FROM rust:1-bookworm AS builder

WORKDIR /app

# Install libraries required by SQLx to compile
RUN apt-get update && \
    apt-get install -y \ pkg-config \libssl-dev \libpq-dev \ clang \
    llvm \ ca-certificates \ libxml2-dev \
    libclang-dev && \
    rm -rf /var/lib/apt/lists/*

# Use nightly for edition 2024
RUN rustup default stable

# Install SQLX CLI
RUN cargo install --version="~0.7" sqlx-cli --no-default-features --features rustls,postgres

# Copy cargo files for caching
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

# Install SSL and postgres runtime libs
RUN apt-get update && \
    apt-get install -y ca-certificates libssl-dev libpq5 \ca-certificates \
        wget && \
    rm -rf /var/lib/apt/lists/*

# Copy compiled server
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Copy SQLX CLI (glibc-compatible now)
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx

# Copy migrations
COPY --from=builder /app/migrations /app/migrations

ENV PORT=8080
EXPOSE 8080

CMD sh -c "sqlx migrate run && ./stc-server"