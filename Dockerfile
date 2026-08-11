FROM rust:1.85-slim-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/kairos-connect-doc-parser /usr/local/bin/doc-parser
EXPOSE 3000
CMD ["doc-parser"]
