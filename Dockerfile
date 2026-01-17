########## 1. Builder Stage ##########
FROM rust:1-bookworm AS builder

WORKDIR /app

RUN apt-get update && \
    apt-get install -y \
        pkg-config \
        clang \
        llvm-14 \
        libclang-14-dev \
        libssl-dev \
        libpq-dev \
        libxml2-dev \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ENV LIBCLANG_PATH=/usr/lib/llvm-14/lib

RUN rustup default nightly

# Install SQLX CLI (if you need it in the builder stage)
RUN cargo install --version="~0.7" sqlx-cli --no-default-features --features rustls,postgres

# Copy cargo files for caching
COPY Cargo.toml Cargo.lock ./

# Pre-build dependencies (small main placeholder)
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy full project
COPY . .

# Build final binary (release)
RUN cargo build --release


########## 2. Runtime Stage ##########
FROM debian:bookworm-slim

WORKDIR /app

# Install only runtime libraries
RUN apt-get update && \
    apt-get install -y \
        ca-certificates \
        libssl3 \
        libpq5 \
        libxml2 \
    && rm -rf /var/lib/apt/lists/*

# Copy compiled server binary from builder
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Copy SQLX CLI (optional)
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx

# Copy migrations
COPY --from=builder /app/migrations /app/migrations

EXPOSE 8080

ENV RUST_BACKTRACE=full

# Run migrations but don't let a migration failure kill the container.
# Then exec the server so it becomes PID 1 and receives signals properly.
# CMD sh -c "sqlx migrate run || echo 'migrations skipped'; exec ./stc-server"
CMD ["sh", "-c", "echo 'starting stc-server'; exec ./stc-server"]

