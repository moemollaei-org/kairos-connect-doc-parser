# kairos-connect-doc-parser

Rust microservice wrapping [anydoc](https://github.com/firecrawl/anydoc) + **Tesseract OCR** — converts 14+ document formats **and images** to GitHub-Flavored Markdown with structured JSON output. Async, streaming, API-key authenticated.

**URL:** `https://parse-doc.thekairos.app`  
**Auth:** `X-Api-Key` header  
**Limits:** 200 MB body, 50 files/request, 16 concurrent  

---

## What's New in v0.2.0

- **OCR support** — extract text from images (PNG, JPEG, GIF, BMP, TIFF, WebP) and scanned/image-based PDF pages
- **Structured JSON output** — every response now includes a `json` field with per-page content, OCR confidence scores, and format metadata
- **Automatic format detection** — images are detected by magic bytes and routed to OCR; PDFs use anydoc for text + OCR for image pages
- **Configurable OCR** — language, DPI, timeout all configurable via env vars
- **Health endpoint** now reports OCR availability

---

## Supported Formats

| Category | Formats | Method |
|----------|---------|--------|
| Word | `.doc` `.docx` `.docm` | anydoc |
| PowerPoint | `.ppt` `.pps` `.pot` `.pptx` `.pptm` `.ppsx` `.ppsm` | anydoc |
| Excel | `.xls` `.xlsx` `.xlsm` `.xlsb` | anydoc |
| OpenDocument | `.odt` `.ods` `.odp` | anydoc |
| Other Docs | `.rtf` `.epub` `.csv` `.pdf` | anydoc |
| **Images** | `.png` `.jpg` `.jpeg` `.gif` `.bmp` `.tiff` `.webp` | **Tesseract OCR** |
| **Scanned PDFs** | `.pdf` (image-based pages) | **pdftoppm → Tesseract OCR** |

---

## Quick Start

### cURL — Image OCR

```bash
curl -X POST https://parse-doc.thekairos.app/convert/raw \
  -H "X-Api-Key: $DOC_PARSER_API_KEY" \
  -H "X-Doc-Filename: receipt.jpg" \
  -H "Content-Type: application/octet-stream" \
  --data-binary @receipt.jpg
```

Response:
```json
{
  "index": 0,
  "filename": "receipt.jpg",
  "markdown": "ACME STORE\n123 Main St\nTotal: $42.99\n",
  "json": {
    "pages": [{"page": 0, "type": "ocr", "content": "ACME STORE\n123 Main St\nTotal: $42.99", "confidence": 0.92}],
    "has_ocr": true,
    "ocr_page_indices": [0],
    "format": "image",
    "page_count": 1,
    "ocr_confidence": [0.92]
  },
  "elapsed_ms": 340
}
```

### cURL — Scanned PDF with OCR

```bash
curl -X POST https://parse-doc.thekairos.app/convert \
  -H "X-Api-Key: $DOC_PARSER_API_KEY" \
  -F "file=@scanned-invoice.pdf"
```

Response (NDJSON stream):
```json
{"index":0,"filename":"scanned-invoice.pdf","markdown":"\n\n## Page 1 (OCR)\n\nINVOICE #1234\nDate: 2024-01-15\nAmount: $1,250.00","json":{"pages":[{"page":0,"type":"ocr","content":"INVOICE #1234\nDate: 2024-01-15\nAmount: $1,250.00","confidence":0.89}],"has_ocr":true,"ocr_page_indices":[0],"format":"pdf","page_count":1,"ocr_confidence":[0.89]},"elapsed_ms":820}
```

### cURL — Disable OCR

```bash
curl -X POST "https://parse-doc.thekairos.app/convert?ocr=false" \
  -H "X-Api-Key: $DOC_PARSER_API_KEY" \
  -F "file=@report.docx"
```

---

## Endpoints

### `GET /health`

No auth. Returns service status and OCR availability.

```bash
curl https://parse-doc.thekairos.app/health
```

```json
{"status":"ok","version":"0.2.0","ocr_available":true}
```

### `POST /convert`

**Auth required.** Multipart file upload. Returns NDJSON stream.

| Query Param | Type | Default | Description |
|-------------|------|---------|-------------|
| `?format=csv` | string | — | Force format for signature-less files |
| `?ocr=false` | boolean | `true` | Disable OCR for this request |

Response: `application/x-ndjson` — one `ConvertResult` JSON per line.

### `POST /convert/raw`

**Auth required.** Single file as raw bytes. Returns single JSON object.

| Header | Required | Description |
|--------|----------|-------------|
| `X-Api-Key` | Yes | API key |
| `X-Doc-Filename` | No | Filename for response |
| `X-Doc-Format` | No | Force format hint (e.g. `csv`) |
| `Content-Type` | Yes | `application/octet-stream` |

| Query Param | Type | Default | Description |
|-------------|------|---------|-------------|
| `?ocr=false` | boolean | `true` | Disable OCR |

---

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `index` | number | 0-based file index |
| `filename` | string | Original filename |
| `markdown` | string \| null | GitHub-Flavored Markdown (anydoc + OCR combined) |
| `json` | object \| null | Structured output with per-page content |
| `json.pages[]` | array | Per-page content (page index, type, content, confidence) |
| `json.has_ocr` | boolean | Whether OCR was performed |
| `json.ocr_page_indices[]` | array | 0-based indices of OCR'd pages |
| `json.format` | string | `document`, `pdf`, or `image` |
| `json.page_count` | number | Total pages |
| `json.ocr_confidence[]` | array | Per-page OCR confidence (0.0-1.0) |
| `error` | string \| null | Error message if conversion failed |
| `elapsed_ms` | number | Total processing time in ms |

---

## Error Codes

| HTTP | Meaning |
|------|---------|
| `200` | Success |
| `400` | Bad request — no files, invalid format, malformed multipart |
| `401` | Missing or wrong `X-Api-Key` |
| `413` | Request body exceeds 200 MB limit |
| `422` | Document is encrypted, unsupported, corrupted, or OCR failed |
| `500` | Internal server error |

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DOC_PARSER_API_KEY` | (required) | API key for auth |
| `PORT` | `3000` | Server listen port |
| `DOC_PARSER_MAX_CONCURRENT` | `16` | Max simultaneous conversions |
| `DOC_PARSER_MAX_BODY_SIZE` | `200MB` | Max request body (supports `MB`, `GB`, raw bytes) |
| `DOC_PARSER_OCR_LANGUAGES` | `eng` | Tesseract language codes (e.g. `eng+spa+fra`) |
| `DOC_PARSER_OCR_DPI` | `300` | DPI for PDF page rendering before OCR |
| `DOC_PARSER_OCR_TIMEOUT_SECS` | `60` | Timeout per OCR page operation |
| `DOC_PARSER_TESSERACT_BIN` | `tesseract` | Path to tesseract binary |
| `DOC_PARSER_PDFTOPPM_BIN` | `pdftoppm` | Path to pdftoppm binary |
| `RUST_LOG` | `info` | Log level |

---

## Docker

```dockerfile
FROM debian:bookworm-slim

# Runtime deps: Tesseract (OCR) + Poppler (PDF rendering)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    tesseract-ocr tesseract-ocr-eng \
    poppler-utils \
    && rm -rf /var/lib/apt/lists/*

# Add more language packs as needed:
# RUN apt-get install -y tesseract-ocr-spa tesseract-ocr-fra

RUN curl -fsSL -o /usr/local/bin/doc-parser \
    "https://github.com/moemollaei-org/kairos-connect-doc-parser/releases/download/v0.2.0/kairos-connect-doc-parser" \
    && chmod +x /usr/local/bin/doc-parser

EXPOSE 3000
CMD ["doc-parser"]
```

### Multi-language OCR

Add additional Tesseract language packs to the Dockerfile:

```dockerfile
RUN apt-get install -y \
    tesseract-ocr-spa \
    tesseract-ocr-fra \
    tesseract-ocr-deu \
    tesseract-ocr-chi-sim
```

Then set `DOC_PARSER_OCR_LANGUAGES=eng+spa+fra` for multi-language detection.

---

## Architecture

```
┌─────────────┐     ┌──────────────────┐     ┌──────────────┐
│  HTTP POST   │────▶│  Format Detect   │────▶│  Image? OCR  │
│  multipart   │     │  (magic bytes)   │     │  PDF? anydoc │
│  / raw bytes │     └──────────────────┘     │  + OCR pages │
└─────────────┘                               └──────┬───────┘
                                                     │
              ┌──────────────────────────────────────┤
              ▼                                      ▼
    ┌─────────────────┐                  ┌──────────────────┐
    │  anydoc         │                  │  pdftoppm        │
    │  (text-based    │                  │  (PDF → PNG      │
    │   documents)    │                  │   per page)      │
    └────────┬────────┘                  └────────┬─────────┘
             │                                    │
             ▼                                    ▼
    ┌─────────────────┐                  ┌──────────────────┐
    │  Markdown       │                  │  Tesseract OCR   │
    │  (GFM)          │                  │  (stdin stdout)  │
    └────────┬────────┘                  └────────┬─────────┘
             │                                    │
             └──────────────┬─────────────────────┘
                            ▼
                   ┌─────────────────┐
                   │  Merge Results  │
                   │  → NDJSON stream│
                   │  (markdown+json)│
                   └─────────────────┘
```

---

## OpenAPI / Swagger

See [openapi.yaml](./openapi.yaml) for the complete OpenAPI 3.0 spec. You can preview it at [editor.swagger.io](https://editor.swagger.io) or use with any OpenAPI-compatible tool.

---

## Deployment Info

| | |
|---|---|
| **Service** | `kairos-connect-doc-parser` on [Railway](https://railway.com) |
| **Project** | Kairos Connect |
| **GitHub** | `moemollaei-org/kairos-connect-doc-parser` |
| **Rust** | 1.88, axum 0.8, tokio, anydoc 0.1, image 0.25 |
| **Runtime deps** | Tesseract OCR, Poppler Utils |
| **Limits** | 200 MB body, configurable via `DOC_PARSER_MAX_BODY_SIZE` |
