# Debian 13 (trixie) ships glibc 2.41. The release binary is built on
# ubuntu-latest (24.04 / glibc 2.39); bookworm-slim only has 2.36, so the
# binary died at exec with: version `GLIBC_2.39" not found.
FROM debian:trixie-slim

# Install runtime dependencies: tesseract (OCR) and poppler-utils (PDF rendering).
#
# tesseract-ocr-all pulls every language pack (~100 languages, adds roughly
# 800MB to the image). That size buys correctness rather than convenience:
# with eng alone, non-Latin scripts do not fail — they return confident
# garbage. A Chinese page came back as "OLS Hiss #20 BR 2SEMS", Russian as
# "Kanpoc KoHHekT OOO". Text that looks extracted but is nonsense is far worse
# than an explicit error, because nothing downstream can tell the difference.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    tesseract-ocr \
    tesseract-ocr-all \
    poppler-utils \
    && rm -rf /var/lib/apt/lists/*

# Download latest release binary (CI/CD uploads it to GitHub Releases).
#
# The cache problem: a bare `RUN curl .../latest/...` has a cache key derived
# only from the instruction text, which never changes — so Docker reuses the
# layer and every deploy silently ships the PREVIOUS binary. That is not
# hypothetical; it served stale code for hours with every check green.
#
# The first fix used `ADD <github-api-url>` to bust the cache. Do not do that:
# every build after it wedged at "scheduling build on Metal builder" and never
# started, while the last build before it succeeded. Railway's Metal builder
# does not cope with a remote ADD.
#
# COPY of a file from the build context is plain, local, and deterministic.
# CD writes the deployed commit into .deploy-sha before `railway up`, so the
# layer hash changes exactly when the code does.
COPY .deploy-sha /etc/doc-parser-build-sha

RUN curl -fsSL -o /usr/local/bin/doc-parser \
    "https://github.com/moemollaei-org/kairos-connect-doc-parser/releases/latest/download/kairos-connect-doc-parser" \
    && chmod +x /usr/local/bin/doc-parser

EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

CMD ["doc-parser"]
