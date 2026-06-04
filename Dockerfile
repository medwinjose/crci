FROM rust:1.76-slim
WORKDIR /app
COPY . .
RUN cargo build --release
