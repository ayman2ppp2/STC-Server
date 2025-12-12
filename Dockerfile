# 1. Build stage
FROM rust:1 as builder

WORKDIR /app

# Copy manifest first for build caching
COPY Cargo.toml Cargo.lock ./

# Dummy file to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies
RUN cargo build --release
RUN rm -rf src

# Copy actual project
COPY . .

# Use nightly if required by edition = "2024"
RUN rustup default nightly

# Build your application
RUN cargo build --release


# 2. Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies + SQLX CLI (for migrations)
RUN apt-get update && \
    apt-get install -y ca-certificates pkg-config libssl-dev && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Install SQLX CLI for postgres + rustls
RUN apt-get update && \
    apt-get install -y curl && \
    curl https://sh.rustup.rs -sSf | sh -s -- -y && \
    /root/.cargo/bin/cargo install sqlx-cli --no-default-features --features rustls,postgres && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy built binary
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Copy migrations
COPY --from=builder /app/migrations /app/migrations

ENV PORT=8080
EXPOSE 8080

# Entry point: run migrations then start the server
CMD sh -c "/root/.cargo/bin/sqlx migrate run && ./stc-server"
