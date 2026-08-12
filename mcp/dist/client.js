import { readFile, stat } from 'node:fs/promises';
import { basename } from 'node:path';
import { CLOUDFLARE_UPLOAD_LIMIT_BYTES, } from './config.js';
/** Thin typed client over the doc-parser HTTP API. */
export class DocParserClient {
    config;
    constructor(config) {
        this.config = config;
    }
    async health() {
        const res = await this.fetchWithTimeout(`${this.config.baseUrl}/health`, {
            method: 'GET',
        });
        if (!res.ok) {
            throw new Error(`Health check failed: HTTP ${res.status}`);
        }
        return (await res.json());
    }
    /**
     * POST /convert — multipart upload of one or more files.
     *
     * The response is NDJSON: one ConvertResult per line, emitted as each file
     * finishes. A per-file failure arrives as a line with `error` set while the
     * HTTP status stays 200, so callers must inspect every line rather than
     * trusting the status code alone.
     */
    async convert(paths, options = {}) {
        if (paths.length === 0) {
            throw new Error('No files given.');
        }
        await this.assertUploadFits(paths);
        const form = new FormData();
        for (const path of paths) {
            const bytes = await readFile(path);
            form.append('file', new Blob([new Uint8Array(bytes)]), basename(path));
        }
        const url = new URL(`${this.config.baseUrl}/convert`);
        if (options.ocr === false)
            url.searchParams.set('ocr', 'false');
        if (options.format)
            url.searchParams.set('format', options.format);
        const res = await this.fetchWithTimeout(url.toString(), {
            method: 'POST',
            headers: { 'X-Api-Key': this.config.apiKey },
            body: form,
        });
        return this.parseNdjson(res);
    }
    /** POST /convert/raw — single file as a raw body, no multipart framing. */
    async convertRaw(path, options = {}) {
        await this.assertUploadFits([path]);
        const bytes = await readFile(path);
        const url = new URL(`${this.config.baseUrl}/convert/raw`);
        if (options.ocr === false)
            url.searchParams.set('ocr', 'false');
        const headers = {
            'X-Api-Key': this.config.apiKey,
            'Content-Type': 'application/octet-stream',
            'X-Doc-Filename': options.filename ?? basename(path),
        };
        if (options.format)
            headers['X-Doc-Format'] = options.format;
        const res = await this.fetchWithTimeout(url.toString(), {
            method: 'POST',
            headers,
            body: new Uint8Array(bytes),
        });
        if (!res.ok) {
            throw new Error(await this.describeFailure(res));
        }
        return (await res.json());
    }
    async assertUploadFits(paths) {
        let total = 0;
        for (const path of paths) {
            const info = await stat(path).catch(() => null);
            if (!info)
                throw new Error(`File not found: ${path}`);
            total += info.size;
        }
        if (total > CLOUDFLARE_UPLOAD_LIMIT_BYTES) {
            const mb = (n) => (n / 1048576).toFixed(1);
            throw new Error(`Upload is ${mb(total)} MB across ${paths.length} file(s), over the ` +
                `${mb(CLOUDFLARE_UPLOAD_LIMIT_BYTES)} MB Cloudflare limit in front of ` +
                `the service. Cloudflare rejects this with an HTML 413 before it ` +
                `reaches the parser. Split the batch into smaller requests.`);
        }
    }
    async parseNdjson(res) {
        const body = await res.text();
        if (!res.ok) {
            throw new Error(await this.describeFailure(res, body));
        }
        const results = [];
        for (const line of body.split('\n')) {
            const trimmed = line.trim();
            if (!trimmed)
                continue;
            try {
                results.push(JSON.parse(trimmed));
            }
            catch {
                throw new Error(`Malformed NDJSON line from the parser: ${trimmed.slice(0, 200)}`);
            }
        }
        return results;
    }
    /** Turn a non-2xx response into a message that names the likely culprit. */
    async describeFailure(res, body) {
        const text = body ?? (await res.text().catch(() => ''));
        if (res.status === 413) {
            return ('HTTP 413: the upload was rejected before reaching the parser — ' +
                'this is the 100 MB Cloudflare limit, not the service. Send fewer ' +
                'or smaller files per request.');
        }
        if (res.status === 401) {
            return 'HTTP 401: DOC_PARSER_API_KEY is missing or wrong.';
        }
        const snippet = text.slice(0, 300).replace(/\s+/g, ' ').trim();
        return `HTTP ${res.status}${snippet ? `: ${snippet}` : ''}`;
    }
    async fetchWithTimeout(url, init) {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), this.config.requestTimeoutMs);
        try {
            return await fetch(url, { ...init, signal: controller.signal });
        }
        catch (err) {
            if (err instanceof Error && err.name === 'AbortError') {
                throw new Error(`Request timed out after ${this.config.requestTimeoutMs} ms. ` +
                    'Large scanned PDFs are slow to OCR — raise DOC_PARSER_TIMEOUT_MS.');
            }
            throw err;
        }
        finally {
            clearTimeout(timer);
        }
    }
}
