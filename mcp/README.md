# Kairos Doc Parser — MCP server

Exposes the whole [document parser API](../openapi.yaml) as MCP tools, so an
agent can convert local documents to markdown/JSON with OCR.

## Tools

| Tool | Wraps | Purpose |
|---|---|---|
| `doc_parser_health` | `GET /health` | Service status, version, whether OCR is available. No API key needed. |
| `convert_documents` | `POST /convert` | Convert **one or many** local files in a single multipart request. |
| `convert_document_raw` | `POST /convert/raw` | Convert exactly one file via the raw-body endpoint (no multipart framing). |
| `inspect_documents` | `POST /convert` | Page counts, format, which pages needed OCR, and timings — **without** pulling document text into context. |

`inspect_documents` has no direct HTTP counterpart; it is the same call as
`convert_documents` with the text withheld. Extracted text runs to tens of
thousands of characters, and triaging a batch should not cost that context.

## Setup

```bash
cd mcp && bun install && bun run build
export DOC_PARSER_API_KEY=...      # matches DOC_PARSER_API_KEY on the service
bun run smoke                      # verifies the server starts and the API answers
```

A project-scoped [`.mcp.json`](../.mcp.json) is checked in at the repo root. It
reads the key from the environment — **do not inline the secret there**, the
file is committed.

To register it globally instead:

```json
{
  "mcpServers": {
    "kairos-doc-parser": {
      "command": "node",
      "args": ["/absolute/path/to/kairos-connect-doc-parser/mcp/dist/index.js"],
      "env": { "DOC_PARSER_API_KEY": "..." }
    }
  }
}
```

## Environment

| Variable | Default | Meaning |
|---|---|---|
| `DOC_PARSER_API_KEY` | *(required)* | Sent as `X-Api-Key`. |
| `DOC_PARSER_BASE_URL` | `https://parse-doc.thekairos.app` | Point at a local instance for development. |
| `DOC_PARSER_TIMEOUT_MS` | `900000` | Raise for very large scanned PDFs. |

## Things worth knowing

**Uploads are capped at 100 MB per request by Cloudflare, not by the service.**
The origin's own limit is 200 MB (`DOC_PARSER_MAX_BODY_SIZE`), but the proxy in
front of `parse-doc.thekairos.app` rejects larger bodies with an HTML `413`
that looks nothing like this API's JSON errors. The client checks the total
before uploading and fails with a clear message instead. Measured: 80.7 MB
across 5 files succeeds; 116.9 MB across 6 fails at the edge in 0.1 s.

**A per-file failure still returns HTTP 200.** `/convert` streams NDJSON, one
result per line, and a bad file arrives as a line with `error` set. Every line
is inspected rather than trusting the status code.

**`output` controls how much lands in context.** `markdown` (default) is the
readable text; `json` adds structure plus per-page content and confidence;
`both` gives structure plus markdown. `max_chars` truncates per document and
says so explicitly, so a truncated read is never mistaken for a short document.

**OCR language is server-side.** The service runs Tesseract with
`DOC_PARSER_OCR_LANGUAGES` (default `eng`); there is no per-request override.
Dutch documents still extract well, but adding `nld` to that variable would
improve accuracy on diacritics and Dutch-specific words.
