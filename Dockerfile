# Build Stage
FROM rust:1.84-slim-bullseye AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    make \
    gcc \
    && rm -rf /var/lib/apt/lists/*


# Install sqlx-cli
RUN cargo install sqlx-cli --no-default-features --features native-tls,sqlite

# Copy entire project
COPY . .

# Set DATABASE_URL and prepare sqlx
ENV DATABASE_URL="sqlite:///app/data/operations.db"
RUN cargo build --release

# Runtime Stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    chromium \
    fonts-freefont-ttf \
    libssl1.1 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -ms /bin/bash appuser

WORKDIR /app
RUN mkdir /app/data && chown -R appuser:appuser /app

# Copy binary
COPY --from=builder --chown=appuser:appuser /app/target/release/pdf_service /usr/local/bin/pdf_service

VOLUME /app/data
EXPOSE 5022

ENV RUST_LOG=info
USER appuser

CMD ["/usr/local/bin/pdf_service"]