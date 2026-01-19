# Builder stage: compile Rust binary with dependencies
FROM rust:bookworm AS builder
# Install system libraries for bindgen, SQLx, libxml2, OpenSSL, etc.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential clang libclang-dev libxml2-dev pkg-config libssl-dev \
    curl git ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Cache Rust dependencies by building a dummy main first0.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release

# Now copy the actual source code and build the real binary.
COPY . .
# If using SQLx query macros, enable offline mode and prepare the query cache1.
RUN cargo install sqlx-cli --version "^0.7" \
 && export SQLX_OFFLINE=true \
 && cargo sqlx prepare \
 && cargo build --release

# Final stage: smaller runtime image with debugging tools included
FROM debian:bookworm-slim AS runtime
# Expose the application port (set via PORT env; Render auto-detects it)2.
ENV PORT 8000
WORKDIR /app

# Install runtime libraries: SSL (for openssl crate), libpq, libxml2, etc.
# Also install gdb for debugging (remove later if slimming).
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 libpq5 libxml2 libclang-dev curl ca-certificates gdb \
 && rm -rf /var/lib/apt/lists/*

# Set LIBCLANG_PATH so bindgen (libxml) can find the clang library3.
ENV LIBCLANG_PATH=/usr/lib/llvm-14/lib

# Copy the compiled binary from the builder stage.
COPY --from=builder /usr/src/app/target/release /app/myapp

# Expose port and set the default command (Render will route $PORT to this).
EXPOSE $PORT
CMD ["./stc-server"]