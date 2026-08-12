#!/usr/bin/env node
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { DocParserClient } from './client.js';
import { loadConfig } from './config.js';
import { DEFAULT_MAX_CHARS, renderResults, summarise } from './format.js';
const config = loadConfig();
const client = new DocParserClient(config);
const server = new McpServer({
    name: 'kairos-doc-parser',
    version: '0.1.0',
});
/** Wrap a handler so failures come back as tool errors, not transport crashes. */
async function toolResult(run) {
    try {
        return { content: [{ type: 'text', text: await run() }] };
    }
    catch (err) {
        return {
            content: [
                {
                    type: 'text',
                    text: `Error: ${err instanceof Error ? err.message : String(err)}`,
                },
            ],
            isError: true,
        };
    }
}
const outputMode = z
    .enum(['markdown', 'json', 'both'])
    .default('markdown')
    .describe("'markdown' for readable text, 'json' for structure plus per-page content, 'both' for structure plus markdown");
const maxChars = z
    .number()
    .int()
    .positive()
    .default(DEFAULT_MAX_CHARS)
    .describe('Per-document character cap before truncation');
server.registerTool('doc_parser_health', {
    title: 'Document parser health',
    description: 'Check the document parser service: status, version, and whether OCR (tesseract) is available on the server. Needs no API key.',
    inputSchema: {},
    annotations: { readOnlyHint: true, openWorldHint: true },
}, async () => toolResult(async () => {
    const h = await client.health();
    return `status: ${h.status}\nversion: ${h.version}\nocr_available: ${h.ocr_available}\nbase_url: ${config.baseUrl}`;
}));
server.registerTool('convert_documents', {
    title: 'Convert documents to markdown/JSON',
    description: 'Convert one or more local documents to markdown and structured JSON, with OCR for scanned pages. ' +
        'Supports pdf, docx, doc, pptx, xlsx, csv, epub, rtf, odt, ods, odp and images (png, jpg, gif, bmp, tiff, webp). ' +
        'Pass several paths to convert them in a single request. ' +
        'Total upload must stay under 100 MB — Cloudflare rejects larger batches before they reach the service.',
    inputSchema: {
        paths: z
            .array(z.string())
            .min(1)
            .describe('Absolute paths of local files to convert'),
        output: outputMode,
        ocr: z
            .boolean()
            .default(true)
            .describe('Run OCR on images and image-based PDF pages. Set false to use the embedded text layer only (much faster, empty for scans).'),
        format: z
            .enum(['csv', 'pdf', 'docx', 'xlsx', 'pptx'])
            .optional()
            .describe('Force a format for files with no detectable signature'),
        max_chars: maxChars,
    },
    annotations: { readOnlyHint: true, openWorldHint: true },
}, async ({ paths, output, ocr, format, max_chars }) => toolResult(async () => {
    const results = await client.convert(paths, { ocr, format });
    return renderResults(results, output, max_chars);
}));
server.registerTool('convert_document_raw', {
    title: 'Convert a single document (raw upload)',
    description: 'Convert exactly one local document via the raw-body endpoint, skipping multipart framing. ' +
        'Equivalent output to convert_documents; useful for a single large file or when multipart is awkward.',
    inputSchema: {
        path: z.string().describe('Absolute path of the local file to convert'),
        output: outputMode,
        ocr: z.boolean().default(true).describe('Run OCR on image-based pages'),
        format: z
            .enum(['csv', 'pdf', 'docx', 'xlsx', 'pptx'])
            .optional()
            .describe('Force a format for files with no detectable signature'),
        filename: z
            .string()
            .optional()
            .describe('Override the filename reported to the parser'),
        max_chars: maxChars,
    },
    annotations: { readOnlyHint: true, openWorldHint: true },
}, async ({ path, output, ocr, format, filename, max_chars }) => toolResult(async () => {
    const result = await client.convertRaw(path, { ocr, format, filename });
    return renderResults([result], output, max_chars);
}));
server.registerTool('inspect_documents', {
    title: 'Inspect documents without returning their text',
    description: 'Report page counts, detected format, which pages needed OCR, and timing for one or more documents — without pulling the extracted text into context. ' +
        'Use this to triage a batch before deciding what to read in full.',
    inputSchema: {
        paths: z
            .array(z.string())
            .min(1)
            .describe('Absolute paths of local files to inspect'),
        ocr: z.boolean().default(true).describe('Run OCR on image-based pages'),
    },
    annotations: { readOnlyHint: true, openWorldHint: true },
}, async ({ paths, ocr }) => toolResult(async () => {
    const results = await client.convert(paths, { ocr });
    return summarise(results);
}));
const transport = new StdioServerTransport();
await server.connect(transport);
