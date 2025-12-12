########## 1. Builder Stage ##########
FROM rust:1 AS builder

WORKDIR /app

# Install libraries required by SQLx to compile
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev libpq-dev && \
    rm -rf /var/lib/apt/lists/*

# Use nightly for edition 2024
RUN rustup default nightly

# Install SQLX CLI in builder stage
RUN cargo install --version="~0.7" sqlx-cli --no-default-features --features rustls,postgres

# Copy Cargo files first for caching
COPY Cargo.toml Cargo.lock ./

# Create fake src to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy actual source code
COPY . .

# Build final binary
RUN cargo build --release


########## 2. Runtime Stage ##########
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime OpenSSL + Postgres libs
RUN apt-get update && \
    apt-get install -y ca-certificates libssl-dev libpq5 && \
    rm -rf /var/lib/apt/lists/*

# Copy compiled server binary
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Copy SQLX CLI from correct path
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx

# Copy migrations directory
COPY --from=builder /app/migrations /app/migrations

ENV PORT=8080
EXPOSE 8080

# Run migrations then start server
CMD sh -c "sqlx migrate run && ./stc-server"
