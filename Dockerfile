# 1. Build stage
FROM rust:1 as builder

# Install system dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Use nightly version to support edition 2024
RUN rustup default nightly

# Compile in release mode
RUN cargo build --release

# 2. Runtime stage
FROM debian:bookworm-slim

# Needed for Rust binaries
RUN apt-get update && \
    apt-get install -y libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Render sets PORT automatically
ENV PORT=8080
EXPOSE 8080

CMD ["./stc-server"]
