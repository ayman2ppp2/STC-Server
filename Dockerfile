########## 1. Builder Stage ##########
FROM rust:1.92-bookworm AS builder

WORKDIR /app

# Install build dependencies for OpenSSL and PostgreSQL
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        pkg-config \
        clang \
        llvm \
        libssl-dev \
        libpq-dev \
        ca-certificates \
        libxml2-dev \
        libclang-dev \
    && rm -rf /var/lib/apt/lists/*

ENV LIBCLANG_PATH=/usr/lib/x86_64-linux-gnu
# Optimize for faster builds
ENV CARGO_NET_RETRY=10
ENV CARGO_JOBS=4

# Install SQLX CLI for migrations (cached separately)
RUN cargo install sqlx-cli@0.8.6 --no-default-features --features rustls,postgres

# Create dummy main.rs for dependency caching
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Copy cargo files for dependency caching
COPY Cargo.toml ./

# Build dependencies (cached until Cargo.toml changes)
RUN cargo build --release && rm -rf src

# Copy source code (this layer invalidates when source changes)
COPY src ./src
COPY migrations ./migrations

# Build final binary
RUN cargo build --release


########## 2. Runtime Stage ##########
FROM debian:bookworm-slim

WORKDIR /app

# Install essential runtime libraries
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        libssl3 \
        libpq5 \
        ca-certificates \
        wget \
    && rm -rf /var/lib/apt/lists/*

# Copy only what's needed for runtime
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Create non-root user for security
RUN useradd -m -u 1000 appuser && chown -R appuser:appuser /app
USER appuser

EXPOSE 8080

ENV RUST_BACKTRACE=1

# Health check for Render (uses built-in health endpoint)
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:${PORT:-8080}/health_check || exit 1

# Run migrations then start server
CMD sh -c "sqlx migrate run --database-url $DATABASE_URL && exec ./stc-server"
# CMD ["sh", "-c", "echo 'starting stc-server'; exec ./stc-server"]

