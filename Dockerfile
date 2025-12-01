# --- Stage 1: Builder ---
FROM rust:latest AS builder

WORKDIR /usr/src/eth-alive
COPY . .

# Build the binary in release mode
RUN cargo build --release

# --- Stage 2: Runtime ---
# We use a slim Debian image to keep the size down
FROM debian:bookworm-slim

# Install HTTPS certificates and OpenSSL (Required for reqwest)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/eth-alive/target/release/eth-alive /usr/local/bin/eth-alive

RUN mkdir -p /data
WORKDIR /data

# When the container starts, run our binary
ENTRYPOINT ["eth-alive"]