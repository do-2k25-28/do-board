FROM rust:1.94-slim AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-unknown-unknown
RUN curl -fsSL -o /usr/local/bin/tailwindcss \
        https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64 \
    && chmod +x /usr/local/bin/tailwindcss
RUN cargo install dioxus-cli --locked
WORKDIR /app
COPY . .
RUN tailwindcss -i frontend/input.css -o frontend/assets/tailwind.css
WORKDIR /app/frontend
RUN dx build --release

FROM nginx:alpine
COPY --from=builder /app/frontend/dist /usr/share/nginx/html
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
