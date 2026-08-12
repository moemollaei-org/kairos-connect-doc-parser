FROM debian:bookworm-slim

# Install runtime dependencies: tesseract (OCR) and poppler-utils (PDF rendering)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    tesseract-ocr \
    tesseract-ocr-eng \
    poppler-utils \
    && rm -rf /var/lib/apt/lists/*

# Download latest release binary (CI/CD uploads to GitHub Releases)
RUN curl -fsSL -o /usr/local/bin/doc-parser \
    "https://github.com/moemollaei-org/kairos-connect-doc-parser/releases/latest/download/kairos-connect-doc-parser" \
    && chmod +x /usr/local/bin/doc-parser

EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

CMD ["doc-parser"]
