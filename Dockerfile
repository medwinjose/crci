# Stage 1: build
FROM rust:1.78-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# Stage 2: runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/crci-node /usr/local/bin/crci-node
COPY --from=builder /app/target/release/byzantine_agent /usr/local/bin/byzantine_agent
ENTRYPOINT ["crci-node"]
