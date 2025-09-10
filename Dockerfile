# 1. Build stage
FROM rust:1.80 as builder

WORKDIR /app
COPY . .

# Compile for release
RUN cargo build --release

# 2. Runtime stage
FROM debian:bookworm-slim

# Needed for Rust binaries
RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/stc-server /app/stc-server

# Render sets PORT
ENV PORT=8080
EXPOSE 8080

CMD ["./stc-server"]
