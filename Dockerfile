FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL -o /usr/local/bin/doc-parser \
  "https://github.com/moemollaei-org/kairos-connect-doc-parser/releases/download/v0.1.0/kairos-connect-doc-parser" \
  && chmod +x /usr/local/bin/doc-parser
EXPOSE 3000
CMD ["doc-parser"]
