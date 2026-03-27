# Multi-stage build for the braille-engine binary.
# Produces a minimal static binary for Linux containers.
# Used for container-based session persistence with CRIU checkpointing.

# --- Build stage ---
FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

RUN cargo build --release --target x86_64-unknown-linux-musl --bin braille-engine

# --- Runtime stage ---
FROM scratch

COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/braille-engine /braille-engine

ENTRYPOINT ["/braille-engine"]
