FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin && echo "fn main() {}" > src/main.rs && echo "fn main() {}" > src/bin/seed.rs \
    && touch src/lib.rs && cargo build --release --locked && rm -rf src
COPY . .
RUN touch src/main.rs src/lib.rs && cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 app
COPY --from=build /app/target/release/dealflow-api /app/target/release/seed /usr/local/bin/
USER app
ENV PORT=8080 LOG_FORMAT=json
EXPOSE 8080
CMD ["dealflow-api"]
