FROM messense/rust-musl-cross:x86_64-musl

WORKDIR /usr/src/app
COPY . .

# Variables de entorno para la compilación estática
ENV OPENSSL_STATIC=1
ENV OPENSSL_DIR=/usr

# Compilar estáticamente
RUN cargo build --target x86_64-unknown-linux-musl --release