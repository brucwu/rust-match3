# ---- build stage ----
FROM rust:1.85-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates && rm -rf /var/lib/apt/lists/* \
 && rustup target add wasm32-unknown-unknown \
 && curl -sSfL https://github.com/rustwasm/wasm-pack/releases/download/v0.13.1/wasm-pack-v0.13.1-x86_64-unknown-linux-musl.tar.gz | tar xz -C /tmp \
 && mv /tmp/wasm-pack-*/wasm-pack /usr/local/bin/
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo test --release && wasm-pack build --release --target web --out-dir www/pkg --no-typescript
COPY www/index.html www/style.css ./www/

# ---- serve stage ----
FROM nginx:1.27-alpine
ENV PORT=10000
COPY nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --from=build /app/www /usr/share/nginx/html
RUN rm -f /usr/share/nginx/html/pkg/.gitignore
EXPOSE 10000
