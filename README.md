# kairos-connect-doc-parser

Rust microservice wrapping [anydoc](https://github.com/firecrawl/anydoc) — converts 14+ document formats to GitHub-Flavored Markdown. Async, streaming, API-key authenticated.

**URL:** `https://parse-doc.thekairos.app`  
**Auth:** `X-Api-Key` header  
**Limits:** 200 MB body, 50 files/request, 16 concurrent  

---

## Supported Formats

| Category | Formats |
|----------|---------|
| Word | `.doc` `.docx` `.docm` |
| PowerPoint | `.ppt` `.pps` `.pot` `.pptx` `.pptm` `.ppsx` `.ppsm` |
| Excel | `.xls` `.xlsx` `.xlsm` `.xlsb` |
| OpenDocument | `.odt` `.ods` `.odp` |
| Other | `.rtf` `.epub` `.csv` `.pdf` |

---

## Endpoints

### `GET /health`

No auth. Health check.

```bash
curl https://parse-doc.thekairos.app/health
```

```json
{"status":"ok","version":"0.1.0"}
```

---

### `POST /convert`

**Auth required.** Upload one or more files via `multipart/form-data`. Returns `application/x-ndjson` — one JSON object per file, streamed as each conversion completes.

**Query params:**

| Param | Description |
|-------|-------------|
| `?format=csv` | Force format for signature-less files (only needed for CSV) |

**cURL — single file:**

```bash
curl -X POST https://parse-doc.thekairos.app/convert \
  -H "X-Api-Key: *** \
  -F "file=@report.docx"
```

**cURL — multiple files:**

```bash
curl -X POST "https://parse-doc.thekairos.app/convert?format=csv" \
  -H "X-Api-Key: *** \
  -F "file=@report.docx" \
  -F "file=@slides.pptx" \
  -F "file=@data.csv" \
  -F "file=@invoice.pdf"
```

**cURL — CSV (explicit format required):**

```bash
curl -X POST "https://parse-doc.thekairos.app/convert?format=csv" \
  -H "X-Api-Key: *** \
  -F "file=@data.csv"
```

**Response (NDJSON):**

```json
{"index":0,"filename":"report.docx","markdown":"# Title\n\n...","elapsed_ms":4}
{"index":1,"filename":"slides.pptx","markdown":"## Slide 1\n...","elapsed_ms":2}
{"index":2,"filename":"data.csv","error":"Unsupported document format","elapsed_ms":0}
```

Each line is a complete JSON object. The stream ends when all files are done.

---

### `POST /convert/raw`

**Auth required.** Single file as raw bytes. Returns `application/json`.

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-Api-Key` | Yes | API key |
| `X-Doc-Filename` | No | File name for the response |
| `X-Doc-Format` | No | Force format (e.g. `csv`) |
| `Content-Type` | Yes | `application/octet-stream` |

**cURL:**

```bash
curl -X POST https://parse-doc.thekairos.app/convert/raw \
  -H "X-Api-Key: *** \
  -H "X-Doc-Filename: invoice.pdf" \
  -H "Content-Type: application/octet-stream" \
  --data-binary @invoice.pdf
```

**cURL — CSV with format hint:**

```bash
curl -X POST https://parse-doc.thekairos.app/convert/raw \
  -H "X-Api-Key: *** \
  -H "X-Doc-Filename: data.csv" \
  -H "X-Doc-Format: csv" \
  -H "Content-Type: application/octet-stream" \
  --data-binary @data.csv
```

**Response:**

```json
{
  "index": 0,
  "filename": "invoice.pdf",
  "markdown": "# INVOICE\n\n| Bill to | Amount |\n| --- | --- |\n| Kairos | $737.87 |\n",
  "elapsed_ms": 8
}
```

---

## Error Codes

| HTTP | Meaning |
|------|---------|
| `200` | Success |
| `400` | Bad request — no files, invalid format, malformed multipart |
| `401` | Missing or wrong `X-Api-Key` |
| `413` | Request body exceeds 200 MB limit |
| `422` | Document is encrypted, unsupported, corrupted, or exceeds safety limits |
| `500` | Internal server error |
| `502` | Service unavailable (Railway redeploy in progress) |

Error response body:

```json
{"error": "Unsupported document format"}
```

---

## Limits

| Limit | Value |
|-------|-------|
| Max body size | 200 MB |
| Max per-file size | 200 MB |
| Max files per `/convert` request | 50 |
| Max concurrent conversions | 16 |

---

## Code Examples

### Python

```python
import requests
import json

API = "https://parse-doc.thekairos.app"
KEY = "KAIROSCONNECT"

# Multi-file multipart
resp = requests.post(
    f"{API}/convert?format=csv",
    headers={"X-Api-Key": KEY},
    files=[
        ("file", ("report.docx", open("report.docx", "rb"), "application/octet-stream")),
        ("file", ("data.xlsx", open("data.xlsx", "rb"), "application/octet-stream")),
    ],
)

for line in resp.text.strip().split("\n"):
    result = json.loads(line)
    markdown = result["markdown"]
    print(markdown)

# Single file raw bytes
with open("invoice.pdf", "rb") as f:
    resp = requests.post(
        f"{API}/convert/raw",
        headers={
            "X-Api-Key": KEY,
            "X-Doc-Filename": "invoice.pdf",
            "Content-Type": "application/octet-stream",
        },
        data=f.read(),
    )

result = resp.json()
print(result["markdown"])
```

### JavaScript (Node.js)

```javascript
const API = "https://parse-doc.thekairos.app";
const KEY = "***;

// Multi-file multipart
const form = new FormData();
form.append("file", docxBlob, "report.docx");
form.append("file", xlsxBlob, "data.xlsx");

const resp = await fetch(`${API}/convert?format=csv`, {
  method: "POST",
  headers: { "X-Api-Key": KEY },
  body: form,
});

const text = await resp.text();
const results = text.trim().split("\n").map(JSON.parse);
for (const r of results) {
  console.log(r.filename, r.markdown?.length, r.elapsed_ms);
}

// Single file raw bytes
const pdfBytes = fs.readFileSync("invoice.pdf");
const resp2 = await fetch(`${API}/convert/raw`, {
  method: "POST",
  headers: {
    "X-Api-Key": KEY,
    "X-Doc-Filename": "invoice.pdf",
    "Content-Type": "application/octet-stream",
  },
  body: pdfBytes,
});

const result = await resp2.json();
console.log(result.markdown);
```

### Rust

```rust
use reqwest::multipart;

let client = reqwest::Client::new();
let form = multipart::Form::new()
    .part("file", multipart::Part::bytes(std::fs::read("report.docx")?)
        .file_name("report.docx"))
    .part("file", multipart::Part::bytes(std::fs::read("data.xlsx")?)
        .file_name("data.xlsx"));

let resp = client
    .post("https://parse-doc.thekairos.app/convert")
    .header("X-Api-Key", "KAIROSCONNECT")
    .multipart(form)
    .send()
    .await?;

let text = resp.text().await?;
for line in text.lines() {
    let result: serde_json::Value = serde_json::from_str(line)?;
    println!("{}", result["markdown"]);
}
```

### Go

```go
import (
    "bytes"
    "mime/multipart"
    "net/http"
)

var buf bytes.Buffer
w := multipart.NewWriter(&buf)

fw, _ := w.CreateFormFile("file", "report.docx")
docx, _ := os.ReadFile("report.docx")
fw.Write(docx)

fw, _ = w.CreateFormFile("file", "data.xlsx")
xlsx, _ := os.ReadFile("data.xlsx")
fw.Write(xlsx)

w.Close()

req, _ := http.NewRequest("POST", "https://parse-doc.thekairos.app/convert", &buf)
req.Header.Set("X-Api-Key", "KAIROSCONNECT")
req.Header.Set("Content-Type", w.FormDataContentType())

resp, _ := http.DefaultClient.Do(req)
body, _ := io.ReadAll(resp.Body)

// body contains NDJSON lines
```

---

## Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `index` | number | 0-based file index in the request |
| `filename` | string | Original filename |
| `markdown` | string \| null | Converted GitHub-Flavored Markdown |
| `error` | string \| null | Error message if conversion failed |
| `elapsed_ms` | number | Conversion time in milliseconds |

---

## Deployment Info

| | |
|---|---|
| **Service** | `kairos-connect-doc-parser` on [Railway](https://railway.com) |
| **Project** | Kairos Connect |
| **GitHub** | `moemollaei-org/kairos-connect-doc-parser` |
| **Rust** | 1.88, axum 0.8, tokio, anydoc 0.1.8 |
| **Limits** | 200 MB body, configurable via `DOC_PARSER_MAX_BODY_SIZE` (e.g. `200MB`, `500MB`, `1GB`) |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DOC_PARSER_API_KEY` | (required) | API key for auth |
| `PORT` | `3000` | Server listen port |
| `DOC_PARSER_MAX_CONCURRENT` | `16` | Max simultaneous conversions |
| `DOC_PARSER_MAX_BODY_SIZE` | `200MB` | Max request body (supports `MB`, `GB`, raw bytes) |
| `RUST_LOG` | `info` | Log level |
