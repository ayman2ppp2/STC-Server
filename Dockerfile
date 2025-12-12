########## 1. Builder Stage ##########
FROM rust:1 AS builder

WORKDIR /app

# Install postgres dev headers for SQLx
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev libpq-dev && \
    rm -rf /var/lib/apt/lists/*

# Use nightly for edition 2024
RUN rustup default nightly

# Install sqlx-cli in builder (this works reliably)
RUN cargo install --version="~0.7" sqlx-cli --no-default-features --features rustls,postgres

# Copy project AFTER caching tools/deps
COPY Cargo.toml Cargo.lock ./

# Create fake src for dependency caching
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy real project now
COPY . .

# Run migrations (optional: only for checking)
# RUN sqlx migrate run

# Build final binary
RUN cargo build --release


########## 2. Runtime Stage ##########
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime libraries only
RUN apt-get update && \
    apt-get install -y ca-certificates libssl-dev libpq5 && \
    rm -rf /var/lib/apt/lists/*

# Copy compiled Rust binary
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Copy sqlx-cli binary from builder
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx

# Copy migrations directory
COPY --from=builder /app/migrations /app/migrations

ENV PORT=8080
EXPOSE 8080

# On Render: run migrations THEN start server
CMD sh -c "sqlx migrate run && ./stc-server"
