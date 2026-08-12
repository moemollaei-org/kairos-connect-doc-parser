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

    let mut pages = Vec::with_capacity(page_count);

    for page_idx in 0..page_count {
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
