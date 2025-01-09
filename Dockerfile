FROM rust:1.83.0-alpine

# Instalar dependencias necesarias
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    sqlite-dev \
    sqlite-static

WORKDIR /usr/src/app
COPY . .

# Variables de entorno para la compilación estática
ENV OPENSSL_STATIC=1
ENV OPENSSL_DIR=/usr

RUN cargo build --release