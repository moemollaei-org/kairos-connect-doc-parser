# Debian 13 (trixie) ships glibc 2.41. The release binary is built on
# ubuntu-latest (24.04 / glibc 2.39); bookworm-slim only has 2.36, so the
# binary died at exec with: version `GLIBC_2.39" not found.
FROM debian:trixie-slim

# Install runtime dependencies: tesseract (OCR) and poppler-utils (PDF rendering)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    tesseract-ocr \
    tesseract-ocr-eng \
    poppler-utils \
    && rm -rf /var/lib/apt/lists/*

# Download latest release binary (CI/CD uploads to GitHub Releases).
#
# The ADD below is load-bearing, not decorative. A bare `RUN curl ...latest...`
# has a cache key derived only from the instruction text, which never changes —
# so Docker reuses the cached layer and every deploy silently ships the
# PREVIOUS binary. CI stays green, CD stays green, the deployment reports
# SUCCESS, and production runs stale code.
#
# That is not hypothetical: the 4.5x OCR speedup was published to Releases at
# 18:24:24 UTC and deployed 8s later, yet production still served the old
# binary (79.2s across the test corpus, versus 14.5s for the new one).
#
# `ADD <url>` re-fetches on every build and folds the response into the layer
# hash, so the release metadata changing (a new asset upload bumps updated_at)
# invalidates the curl below. Keep the ADD immediately before the RUN.
ADD https://api.github.com/repos/moemollaei-org/kairos-connect-doc-parser/releases/latest /tmp/release.json

RUN curl -fsSL -o /usr/local/bin/doc-parser \
    "https://github.com/moemollaei-org/kairos-connect-doc-parser/releases/latest/download/kairos-connect-doc-parser" \
    && chmod +x /usr/local/bin/doc-parser \
    && /usr/local/bin/doc-parser --version 2>/dev/null || true

EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

CMD ["doc-parser"]
