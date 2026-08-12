/**
 * Extracted document text is frequently tens of thousands of characters, and
 * an MCP tool result goes straight into a model's context. Cap it by default
 * and say plainly when content was cut, so a truncated read is never mistaken
 * for a short document.
 */
export const DEFAULT_MAX_CHARS = 20_000;
function truncate(text, maxChars) {
    if (text.length <= maxChars)
        return { text, truncated: false };
    return {
        text: `${text.slice(0, maxChars)}\n\n…[truncated ${text.length - maxChars} of ${text.length} characters — raise max_chars to see the rest]`,
        truncated: true,
    };
}
/** One-line-per-file summary: what was parsed, how, and how fast. */
export function summarise(results) {
    const lines = results.map((r) => {
        if (r.error)
            return `- ${r.filename}: ERROR — ${r.error}`;
        const j = r.json;
        const pages = j?.page_count ?? j?.pages?.length ?? '?';
        const ocr = j?.has_ocr
            ? `OCR on page(s) ${(j.ocr_page_indices ?? []).join(', ') || '?'}`
            : 'no OCR (embedded text layer)';
        const chars = r.markdown?.length ?? 0;
        const lang = j?.ocr_languages ? `, lang ${j.ocr_languages}` : '';
        return `- ${r.filename}: ${pages} page(s), ${ocr}${lang}, ${chars} chars, ${r.elapsed_ms} ms`;
    });
    const failed = results.filter((r) => r.error).length;
    const header = `${results.length} file(s) processed` +
        (failed ? `, ${failed} failed` : '') +
        ':';
    return [header, ...lines].join('\n');
}
/** Render results for the model, honouring the requested output mode. */
export function renderResults(results, mode, maxChars) {
    const blocks = [summarise(results), ''];
    for (const r of results) {
        blocks.push(`\n---\n\n# ${r.filename}`);
        if (r.error) {
            blocks.push(`\n**Conversion failed:** ${r.error}`);
            continue;
        }
        if (mode === 'json' || mode === 'both') {
            // Page text is emitted separately under markdown; keeping it here too
            // would double the payload for `both`.
            const meta = { ...r.json, pages: undefined };
            blocks.push('\n## Structure\n\n```json');
            blocks.push(JSON.stringify(meta, null, 2));
            blocks.push('```');
            if (mode === 'json' && r.json?.pages) {
                const pages = r.json.pages.map((p) => ({
                    page: p.page,
                    type: p.type,
                    confidence: p.confidence,
                    content: truncate(p.content, maxChars).text,
                }));
                blocks.push('\n## Pages\n\n```json');
                blocks.push(JSON.stringify(pages, null, 2));
                blocks.push('```');
            }
        }
        if (mode === 'markdown' || mode === 'both') {
            const { text } = truncate(r.markdown ?? '', maxChars);
            blocks.push(`\n## Markdown\n\n${text}`);
        }
    }
    return blocks.join('\n');
}
