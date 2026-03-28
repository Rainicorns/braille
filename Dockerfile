# Multi-stage build for the braille-engine and braille-worker binaries.
# Produces minimal static binaries for Linux containers.
# Used for container-based session persistence with CRIU checkpointing.

# --- Build stage ---
FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

RUN cargo build --release --target x86_64-unknown-linux-musl -p braille-engine

# --- Runtime stage ---
FROM scratch

COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/braille-engine /bin/braille-engine
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/braille-worker /bin/braille-worker

USER 1000

ENTRYPOINT ["/bin/braille-engine"]
