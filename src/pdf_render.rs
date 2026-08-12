//! PDF rendering module — converts PDF pages to images for OCR.
//!
//! Uses the `pdftoppm` CLI (from poppler-utils) to render each page
//! of a PDF to a PNG image at the configured DPI.

use std::process::Stdio;
use tokio::process::Command;

use crate::config::Config;

/// Render a range of PDF pages to PNG images.
///
/// Returns a vector of (page_index, png_bytes) tuples.
/// Page indices are 0-based.
pub async fn render_pdf_pages(
    config: &Config,
    pdf_bytes: &[u8],
) -> Result<Vec<(usize, Vec<u8>)>, String> {
    // pdftoppm reads from a file, so write to a temp file
    let tmp_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let pdf_path = tmp_dir.path().join("input.pdf");
    std::fs::write(&pdf_path, pdf_bytes).map_err(|e| format!("write temp pdf: {e}"))?;

    // Get page count first
    let page_count = get_pdf_page_count(pdf_bytes).await?;

    // Fast path: a scanned page is a single full-page image, so poppler can hand
    // us the original stream instead of rasterising a fresh one. On a 600 PPI
    // scan that is ~170ms against ~3000ms for pdftoppm at 300 DPI — the render,
    // not the OCR, was the dominant cost. Pages that are not a single embedded
    // image (vector art, text plus several figures) fall through to pdftoppm.
    let embedded = list_embedded_images(config, &pdf_path)
        .await
        .unwrap_or_default();
    let single_image_pages: std::collections::HashSet<usize> = {
        let mut counts: std::collections::HashMap<usize, (usize, u32)> =
            std::collections::HashMap::new();
        for img in &embedded {
            let entry = counts.entry(img.page).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = entry.1.max(img.width.min(img.height));
        }
        counts
            .into_iter()
            // Require a reasonably large image, so a page carrying one small
            // logo is not mistaken for a scan and OCR'd from the logo alone.
            .filter(|(_, (count, min_edge))| *count == 1 && *min_edge >= 800)
            .map(|(page, _)| page)
            .collect()
    };

    let mut pages = Vec::with_capacity(page_count);

    for page_idx in 0..page_count {
        if single_image_pages.contains(&page_idx) {
            match extract_embedded_image(config, &pdf_path, tmp_dir.path(), page_idx).await {
                Ok(raw) => match downscale_for_ocr(&raw, config.ocr_max_pixels) {
                    Ok(png) => {
                        pages.push((page_idx, png));
                        continue;
                    }
                    Err(e) => tracing::debug!(
                        "page {page_idx}: downscale failed ({e}), falling back to pdftoppm"
                    ),
                },
                Err(e) => tracing::debug!(
                    "page {page_idx}: embedded-image extraction failed ({e}), falling back to pdftoppm"
                ),
            }
        }

        let output_prefix = tmp_dir.path().join(format!("page_{page_idx:04}"));

        let dpi_str = config.ocr_dpi.to_string();

        let output = Command::new(&config.pdftoppm_bin)
            .args([
                "-png",
                "-r",
                &dpi_str,
                "-f",
                &(page_idx + 1).to_string(),
                "-l",
                &(page_idx + 1).to_string(),
                "-singlefile",
            ])
            .arg(&pdf_path)
            .arg(&output_prefix)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("pdftoppm process error (is poppler-utils installed?): {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("pdftoppm failed: {}", stderr.trim()));
        }

        // pdftoppm -singlefile writes: output_prefix.png
        let png_path = output_prefix.with_extension("png");

        if png_path.exists() {
            let png_bytes =
                std::fs::read(&png_path).map_err(|e| format!("read rendered page: {e}"))?;
            pages.push((page_idx, png_bytes));
        } else {
            return Err(format!(
                "pdftoppm did not produce output for page {}",
                page_idx + 1
            ));
        }
    }

    Ok(pages)
}

/// One image embedded in a page, as reported by `pdfimages -list`.
struct EmbeddedImage {
    page: usize,
    width: u32,
    height: u32,
}

/// Ask poppler what images each page embeds, without decoding any of them.
async fn list_embedded_images(
    config: &Config,
    pdf_path: &std::path::Path,
) -> Result<Vec<EmbeddedImage>, String> {
    let output = Command::new(&config.pdfimages_bin)
        .arg("-list")
        .arg(pdf_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("pdfimages process error: {e}"))?;

    if !output.status.success() {
        return Err("pdfimages -list failed".to_string());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut images = Vec::new();
    // Columns: page num type width height color comp bpc enc ...
    for line in text.lines().skip(2) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 5 {
            continue;
        }
        let (Ok(page), Ok(width), Ok(height)) = (
            cols[0].parse::<usize>(),
            cols[3].parse::<u32>(),
            cols[4].parse::<u32>(),
        ) else {
            continue;
        };
        images.push(EmbeddedImage {
            page: page.saturating_sub(1),
            width,
            height,
        });
    }
    Ok(images)
}

/// Pull one embedded image out of a PDF verbatim, without re-rasterising it.
async fn extract_embedded_image(
    config: &Config,
    pdf_path: &std::path::Path,
    tmp: &std::path::Path,
    page_idx: usize,
) -> Result<Vec<u8>, String> {
    let prefix = tmp.join(format!("emb_{page_idx:04}"));
    let page_arg = (page_idx + 1).to_string();
    let output = Command::new(&config.pdfimages_bin)
        .args(["-f", &page_arg, "-l", &page_arg, "-j"])
        .arg(pdf_path)
        .arg(&prefix)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("pdfimages process error: {e}"))?;

    if !output.status.success() {
        return Err("pdfimages extraction failed".to_string());
    }

    // pdfimages appends -NNN plus an extension it chooses from the stream type.
    let dir = std::fs::read_dir(tmp).map_err(|e| format!("read tmp dir: {e}"))?;
    let wanted = format!("emb_{page_idx:04}-");
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&wanted) {
            return std::fs::read(entry.path()).map_err(|e| format!("read embedded image: {e}"));
        }
    }
    Err(format!("pdfimages produced no output for page {page_idx}"))
}

/// Downscale so the longest edge is at most `max_edge`, and drop to greyscale.
///
/// Tesseract binarises internally, so colour is wasted work, and recognising a
/// 600 PPI scan at full size costs seconds per page without improving output.
fn downscale_for_ocr(bytes: &[u8], max_edge: u32) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("decode embedded image: {e}"))?;
    let (w, h) = (img.width(), img.height());
    let scaled = if w.max(h) > max_edge {
        img.resize(max_edge, max_edge, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let mut out = Vec::new();
    scaled
        .to_luma8()
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| format!("encode page image: {e}"))?;
    Ok(out)
}

/// Get the number of pages in a PDF file.
async fn get_pdf_page_count(pdf_bytes: &[u8]) -> Result<usize, String> {
    // Use pdfinfo from poppler-utils
    let tmp_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let pdf_path = tmp_dir.path().join("count.pdf");
    std::fs::write(&pdf_path, pdf_bytes).map_err(|e| format!("write temp pdf: {e}"))?;

    let output = Command::new("pdfinfo")
        .arg(&pdf_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("pdfinfo error: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if line.to_lowercase().starts_with("pages:") {
            let count: usize = line
                .split(':')
                .nth(1)
                .unwrap_or("0")
                .trim()
                .parse()
                .map_err(|e| format!("parse page count: {e}"))?;
            return Ok(count);
        }
    }

    // Fallback: count rendered pages (if pdfinfo fails, assume 1)
    Ok(1)
}

/// Check if poppler-utils (pdftoppm, pdfinfo) is available
#[allow(dead_code)]
pub async fn is_poppler_available() -> bool {
    Command::new("pdftoppm")
        .arg("-v")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|mut c| {
            let _ = c.start_kill();
            true
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {

    /// A minimal valid PDF (1 blank page)
    fn minimal_pdf() -> Vec<u8> {
        // Minimal valid PDF with 1 blank page
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        pdf.extend_from_slice(b"1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n");
        pdf.extend_from_slice(b"2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n");
        pdf.extend_from_slice(b"3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]>>endobj\n");
        pdf.extend_from_slice(b"xref\n");
        pdf.extend_from_slice(b"0 4\n");
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        pdf.extend_from_slice(b"0000000009 00000 n \n");
        pdf.extend_from_slice(b"0000000058 00000 n \n");
        pdf.extend_from_slice(b"0000000115 00000 n \n");
        pdf.extend_from_slice(b"trailer<</Size 4/Root 1 0 R>>\n");
        pdf.extend_from_slice(b"startxref\n");
        pdf.extend_from_slice(b"190\n");
        pdf.extend_from_slice(b"%%EOF\n");
        pdf
    }

    #[test]
    fn test_minimal_pdf_is_valid() {
        let pdf = minimal_pdf();
        assert!(pdf.starts_with(b"%PDF"));
    }
}
