# ------------------------------------------------------------------------------
# Stage 1: Planner
# ------------------------------------------------------------------------------
FROM rust:bookworm AS planner
WORKDIR /app
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ------------------------------------------------------------------------------
# Stage 2: Cacher (The Heavy Lifting)
# ------------------------------------------------------------------------------
FROM rust:bookworm AS cacher
WORKDIR /app
RUN cargo install cargo-chef
# Install system build-deps
RUN apt-get update && apt-get install -y --no-install-recommends \
    clang libclang-dev libxml2-dev pkg-config libssl-dev
COPY --from=planner /app/recipe.json recipe.json
# This is the layer that will stay cached for ~150s!
RUN cargo chef cook --release --recipe-path recipe.json

# ------------------------------------------------------------------------------
# Stage 3: Builder (The Fast Part)
# ------------------------------------------------------------------------------
FROM rust:bookworm AS builder
WORKDIR /app
COPY . .
# Copy pre-compiled dependencies from cacher
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

# Install system build-deps
# OPTIMIZATION: Cache apt downloads and lists
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    clang libclang-dev libxml2-dev pkg-config libssl-dev
# Build only your app code (should take < 10-20s)
ENV SQLX_OFFLINE=true
ENV LIBCLANG_PATH=/usr/lib/llvm-14/lib
RUN cargo build --release --bin stc-server
# ------------------------------------------------------------------------------
# Stage 4: Runtime
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# 4. SECURITY: Create a non-root user
RUN useradd -ms /bin/bash appuser

WORKDIR /app

# 5. RUNTIME DEPS (CLEANED)
# Removed: libclang-dev, gdb (save for debugging), build tools.
# Added: ca-certificates (for HTTPS), libxml2 (dynamic link).
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 libpq5 libxml2 ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Change ownership to the non-root user
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# 6. CONFIGURATION
ENV PORT=8000
EXPOSE ${PORT}

# 7. EXEC FORM (Better for signals)
CMD ["./stc-server"]