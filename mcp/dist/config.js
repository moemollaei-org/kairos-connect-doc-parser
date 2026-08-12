/**
 * Cloudflare rejects uploads above 100 MB before they ever reach the origin,
 * returning an HTML 413 that looks nothing like this API's JSON errors. The
 * service's own DOC_PARSER_MAX_BODY_SIZE (200 MB default) is therefore
 * unreachable through the proxied domain. Check the total here so the caller
 * gets an actionable message instead of a stray HTML body.
 */
export const CLOUDFLARE_UPLOAD_LIMIT_BYTES = 100 * 1024 * 1024;
export function loadConfig() {
    const apiKey = process.env['DOC_PARSER_API_KEY'] ?? '';
    if (!apiKey) {
        throw new Error('DOC_PARSER_API_KEY is not set. Add it to the MCP server env block.');
    }
    return {
        baseUrl: (process.env['DOC_PARSER_BASE_URL'] ?? 'https://parse-doc.thekairos.app').replace(/\/+$/, ''),
        apiKey,
        requestTimeoutMs: Number(process.env['DOC_PARSER_TIMEOUT_MS'] ?? 900_000),
    };
}
