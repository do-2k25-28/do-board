FROM rust:1.94-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build -p backend --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
EXPOSE 3000
CMD ["backend"]
