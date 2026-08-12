use std::time::Instant;

use crate::config::Config;
use crate::models::{ConvertResult, DocumentJson, PageContent, PageType};
use crate::ocr;
use crate::pdf_render;

pub struct ConvertInput {
    pub index: usize,
    pub filename: String,
    pub bytes: Vec<u8>,
    pub format_hint: Option<anydoc::Format>,
    /// Whether OCR is enabled for this conversion
    pub ocr_enabled: bool,
}

/// CPU-bound conversion, runs in spawn_blocking for anydoc + async for OCR
pub async fn convert_one(config: &Config, input: ConvertInput) -> ConvertResult {
    let filename = input.filename.clone();
    let index = input.index;
    let start = Instant::now();

    // Determine the format
    let detected = anydoc::Format::from_bytes(&input.bytes);
    let format = input.format_hint.or(detected);

    let is_pdf = matches!(format, Some(anydoc::Format::Pdf));
    let is_image = is_image_format(&input.bytes);

    // Determine OCR strategy
    let should_ocr = input.ocr_enabled && (is_image || is_pdf);

    if !should_ocr {
        // Fast path: use anydoc only (original behavior)
        return convert_anydoc_only(index, filename, &input.bytes, format, start).await;
    }

    if is_image && !is_pdf {
        // Direct image → OCR only
        return convert_image_ocr(config, index, filename, &input.bytes, start).await;
    }

    // PDF: anydoc for text + OCR for image-based pages
    convert_pdf_with_ocr(config, index, filename, &input.bytes, format, start).await
}

/// Fast path: anydoc only (no OCR)
async fn convert_anydoc_only(
    index: usize,
    filename: String,
    bytes: &[u8],
    format_hint: Option<anydoc::Format>,
    start: Instant,
) -> ConvertResult {
    let owned = bytes.to_vec();
    let result = tokio::task::spawn_blocking(move || {
        let detected = anydoc::Format::from_bytes(&owned);
        anydoc::to_markdown_bytes(&owned, format_hint.or(detected))
    })
    .await;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(md)) => {
            let json = DocumentJson {
                pages: None,
                has_ocr: false,
                ocr_page_indices: None,
                format: "document".into(),
                page_count: None,
                ocr_confidence: None,
            };
            ConvertResult {
                index,
                filename,
                markdown: Some(md),
                json: Some(json),
                error: None,
                elapsed_ms,
            }
        }
        Ok(Err(e)) => ConvertResult {
            index,
            filename,
            markdown: None,
            json: None,
            error: Some(error_msg(&e)),
            elapsed_ms,
        },
        Err(je) => ConvertResult {
            index,
            filename,
            markdown: None,
            json: None,
            error: Some(format!("Conversion task panicked: {je}")),
            elapsed_ms,
        },
    }
}

/// OCR a standalone image file
async fn convert_image_ocr(
    config: &Config,
    index: usize,
    filename: String,
    bytes: &[u8],
    start: Instant,
) -> ConvertResult {
    let owned = bytes.to_vec();
    let cfg = config.clone();

    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(ocr::ocr_image_bytes(&cfg, &owned))
    })
    .await;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(ocr_result)) => {
            let md = format!("{}\n", ocr_result.text);
            let page = PageContent {
                page: 0,
                content_type: PageType::Ocr,
                content: ocr_result.text.clone(),
                confidence: Some(ocr_result.confidence),
            };
            let json = DocumentJson {
                pages: Some(vec![page]),
                has_ocr: true,
                ocr_page_indices: Some(vec![0]),
                format: "image".into(),
                page_count: Some(1),
                ocr_confidence: Some(vec![ocr_result.confidence]),
            };
            ConvertResult {
                index,
                filename,
                markdown: Some(md),
                json: Some(json),
                error: None,
                elapsed_ms,
            }
        }
        Ok(Err(e)) => ConvertResult {
            index,
            filename,
            markdown: None,
            json: None,
            error: Some(format!("OCR failed: {e}")),
            elapsed_ms,
        },
        Err(je) => ConvertResult {
            index,
            filename,
            markdown: None,
            json: None,
            error: Some(format!("OCR task panicked: {je}")),
            elapsed_ms,
        },
    }
}

/// Convert a PDF: anydoc for text + OCR for scanned/image pages
async fn convert_pdf_with_ocr(
    config: &Config,
    index: usize,
    filename: String,
    bytes: &[u8],
    format_hint: Option<anydoc::Format>,
    start: Instant,
) -> ConvertResult {
    let owned = bytes.to_vec();
    let owned_ocr = bytes.to_vec();
    let cfg = config.clone();
    let cfg_ocr = config.clone();

    // Run anydoc conversion and PDF rendering in parallel
    let anydoc_task = tokio::task::spawn_blocking(move || {
        let detected = anydoc::Format::from_bytes(&owned);
        anydoc::to_markdown_bytes(&owned, format_hint.or(detected))
    });

    let ocr_task = tokio::task::spawn(async move {
        match pdf_render::render_pdf_pages(&cfg_ocr, &owned_ocr).await {
            Ok(pages) => {
                let mut results = Vec::new();
                for (page_idx, png_bytes) in pages {
                    match ocr::ocr_image_bytes(&cfg, &png_bytes).await {
                        Ok(ocr_result) => {
                            results.push((page_idx, Some(ocr_result)));
                        }
                        Err(e) => {
                            tracing::warn!("OCR failed for page {page_idx}: {e}");
                            results.push((page_idx, None));
                        }
                    }
                }
                Ok(results)
            }
            Err(e) => Err(e),
        }
    });

    // Wait for anydoc
    let anydoc_result = anydoc_task.await;

    // Wait for OCR
    let ocr_result = ocr_task.await;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    // Process anydoc result
    let (anydoc_md, anydoc_err) = match anydoc_result {
        Ok(Ok(md)) => (Some(md), None),
        Ok(Err(e)) => (None, Some(error_msg(&e))),
        Err(je) => (None, Some(format!("Conversion task panicked: {je}"))),
    };

    // Process OCR results
    let (pages, ocr_indices, confidence) = match ocr_result {
        Ok(Ok(ocr_pages)) => {
            let mut pages = Vec::new();
            let mut ocr_indices = Vec::new();
            let mut confidence = Vec::new();

            for (page_idx, ocr_opt) in ocr_pages {
                if let Some(ocr) = ocr_opt {
                    pages.push(PageContent {
                        page: page_idx,
                        content_type: PageType::Ocr,
                        content: ocr.text,
                        confidence: Some(ocr.confidence),
                    });
                    ocr_indices.push(page_idx);
                    confidence.push(ocr.confidence);
                } else {
                    pages.push(PageContent {
                        page: page_idx,
                        content_type: PageType::Ocr,
                        content: String::new(),
                        confidence: None,
                    });
                }
            }

            (Some(pages), Some(ocr_indices), Some(confidence))
        }
        _ => (None, Some(Vec::new()), Some(Vec::new())),
    };

    // Build combined markdown: anydoc text + OCR pages
    let markdown = build_combined_markdown(anydoc_md.as_deref(), pages.as_deref());

    let has_ocr = ocr_indices.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
    let page_count = pages.as_ref().map(|p| p.len());

    let json = DocumentJson {
        pages,
        has_ocr,
        ocr_page_indices: ocr_indices,
        format: "pdf".into(),
        page_count,
        ocr_confidence: confidence,
    };

    // If anydoc failed completely but we have OCR, still return results
    let error = if anydoc_md.is_none() && !has_ocr {
        anydoc_err
    } else {
        None
    };

    ConvertResult {
        index,
        filename,
        markdown,
        json: Some(json),
        error,
        elapsed_ms,
    }
}

/// Build a combined markdown document from anydoc output and OCR page content
fn build_combined_markdown(
    anydoc_md: Option<&str>,
    ocr_pages: Option<&[PageContent]>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    // AnyDoc content first
    if let Some(md) = anydoc_md {
        if !md.trim().is_empty() {
            parts.push(md.trim().to_string());
        }
    }

    // OCR pages appended
    if let Some(pages) = ocr_pages {
        for page in pages {
            if !page.content.trim().is_empty() {
                parts.push(format!(
                    "\n\n## Page {} (OCR)\n\n{}",
                    page.page + 1,
                    page.content
                ));
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// Check if bytes represent an image format (not a document)
fn is_image_format(bytes: &[u8]) -> bool {
    // Check magic bytes for common image formats
    if bytes.len() < 4 {
        return false;
    }

    // PNG: 89 50 4E 47
    if &bytes[0..4] == b"\x89PNG" {
        return true;
    }
    // JPEG: FF D8 FF
    if bytes.len() >= 3 && &bytes[0..3] == b"\xff\xd8\xff" {
        return true;
    }
    // GIF: GIF8
    if bytes.len() >= 4 && &bytes[0..4] == b"GIF8" {
        return true;
    }
    // BMP: BM
    if bytes.len() >= 2 && &bytes[0..2] == b"BM" {
        return true;
    }
    // TIFF: 49 49 2A 00 (little-endian) or 4D 4D 00 2A (big-endian)
    if bytes.len() >= 4 && (&bytes[0..4] == b"I I\x2a\x00" || &bytes[0..4] == b"MM\x00\x2a") {
        return true;
    }
    // WebP: RIFF....WEBP
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return true;
    }

    false
}

fn error_msg(e: &anydoc::ConvertError) -> String {
    match e {
        anydoc::ConvertError::Encrypted => "Document is encrypted or password-protected".into(),
        anydoc::ConvertError::Unsupported(_) => "Unsupported document format".into(),
        anydoc::ConvertError::Malformed { .. } => "Document is malformed or corrupt".into(),
        anydoc::ConvertError::ResourceLimit { .. } => "Document exceeds processing limits".into(),
        anydoc::ConvertError::MissingPart { .. } => {
            "Required part of the document is missing".into()
        }
        anydoc::ConvertError::Io(io) => format!("Could not read document: {io}"),
        _ => format!("Conversion error: {e:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_image_format_png() {
        let png = b"\x89PNG\r\n\x1a\nrest of data...";
        assert!(is_image_format(png));
    }

    #[test]
    fn test_is_image_format_jpeg() {
        let jpg = b"\xff\xd8\xff\xe0rest...";
        assert!(is_image_format(jpg));
    }

    #[test]
    fn test_is_image_format_pdf() {
        let pdf = b"%PDF-1.4\nrest...";
        assert!(!is_image_format(pdf));
    }

    #[test]
    fn test_is_image_format_empty() {
        assert!(!is_image_format(&[]));
    }

    #[test]
    fn test_is_image_format_webp() {
        let webp = b"RIFF\x00\x00\x00\x00WEBPrest...";
        assert!(is_image_format(webp));
    }

    #[test]
    fn test_build_combined_markdown_both() {
        let pages = vec![PageContent {
            page: 0,
            content_type: PageType::Ocr,
            content: "Scanned text".into(),
            confidence: Some(0.9),
        }];
        let md = build_combined_markdown(Some("# Title\n\nBody text"), Some(&pages));
        let md = md.unwrap();
        assert!(md.contains("Page 1 (OCR)"));
        assert!(md.contains("# Title"));
    }

    #[test]
    fn test_build_combined_markdown_ocr_only() {
        let pages = vec![PageContent {
            page: 0,
            content_type: PageType::Ocr,
            content: "Only OCR".into(),
            confidence: Some(0.8),
        }];
        let md = build_combined_markdown(None, Some(&pages));
        assert_eq!(md.unwrap().trim(), "## Page 1 (OCR)\n\nOnly OCR");
    }

    #[test]
    fn test_build_combined_markdown_empty() {
        let pages = vec![PageContent {
            page: 0,
            content_type: PageType::Ocr,
            content: String::new(),
            confidence: None,
        }];
        let md = build_combined_markdown(None, Some(&pages));
        assert!(md.is_none());
    }
}
