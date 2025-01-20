# =====================
# Etapa 1: BUILD (Alpine)
# =====================
FROM rust:1.84.0-alpine AS builder

WORKDIR /app

# Add optimization env vars
ENV RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C lto=thin"
ENV CARGO_PROFILE_RELEASE_LTO="true"
ENV CARGO_PROFILE_RELEASE_CODEGEN_UNITS="1"
ENV CARGO_PROFILE_RELEASE_OPT_LEVEL="3"

# Install dependencies
RUN apk add --no-cache \
    build-base \
    musl-dev \
    musl-tools \
    openssl-dev \
    perl \
    pkgconfig

# Add musl target for Rust
RUN rustup target add x86_64-unknown-linux-musl

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl && \
    rm -rf src

# Build application
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl && \
    strip target/x86_64-unknown-linux-musl/release/pdf_service

# =====================
# Etapa 2: RUNTIME (Alpine)
# =====================
FROM alpine:3.18

# Security: Run as non-root
RUN addgroup -S appgroup && adduser -S appuser -G appgroup

# Install runtime dependencies
RUN apk add --no-cache \
    chromium \
    harfbuzz \
    freetype \
    nss \
    ttf-freefont \
    tini

WORKDIR /app
RUN mkdir /app/data && chown -R appuser:appgroup /app

# Copy binary
COPY --from=builder --chown=appuser:appgroup /app/target/x86_64-unknown-linux-musl/release/pdf_service /usr/local/bin/pdf_service

VOLUME /app/data
EXPOSE 5022

ENV RUST_LOG=info
USER appuser

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["pdf_service"]
