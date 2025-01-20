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

# Install dependencies with version pinning
RUN apk add --no-cache \
    build-base=0.5-r3 \
    perl=5.36.1-r2 \
    pkgconfig=1.8.1-r1 \
    musl-dev=1.2.4-r1 \
    openssl-dev=3.1.4-r2 \
    x86_64-linux-musl-cross

# Add musl target
RUN rustup target add x86_64-unknown-linux-musl

ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc

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
    chromium=112.0.5615.165-r0 \
    harfbuzz=7.0.0-r0 \
    freetype=2.12.1-r0 \
    nss=3.89.1-r0 \
    ttf-freefont=20120503-r4 \
    tini=0.19.0-r1

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