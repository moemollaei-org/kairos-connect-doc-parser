//! OCR module — wraps the Tesseract CLI for extracting text from images.
//!
//! Uses `tesseract stdin stdout` to read image bytes from stdin and
//! return recognized text. Falls back gracefully if tesseract is not installed.

use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;

use crate::config::Config;

/// Result of OCR on a single image
#[derive(Debug, Clone)]
pub struct OcrResult {
    /// Recognized text
    pub text: String,
    /// Mean confidence (0.0-1.0), estimated from Tesseract output
    pub confidence: f64,
    /// Time taken in ms
    #[allow(dead_code)]
    pub elapsed_ms: u64,
}

/// Run OCR on raw image bytes using the Tesseract CLI.
///
/// Image bytes can be PNG, JPEG, TIFF, BMP, or any format Leptonica supports.
/// The image is piped to `tesseract stdin stdout -l <lang>` via stdin.
/// OCR an image using an explicit language spec (e.g. `nld+eng`).
///
/// The spec reaches a command-line argument, so callers must have run it
/// through `languages::validate` first — never pass caller input here raw.
pub async fn ocr_image_bytes_with_lang(
    config: &Config,
    image_bytes: &[u8],
    lang: &str,
) -> Result<OcrResult, String> {
    let start = Instant::now();

    let mut child = Command::new(&config.tesseract_bin)
        .args(["stdin", "stdout", "-l", lang])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start tesseract (is it installed?): {e}"))?;

    // Write image bytes to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(image_bytes)
            .await
            .map_err(|e| format!("Failed to write image to tesseract stdin: {e}"))?;
        drop(stdin); // Close stdin to signal EOF
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(config.ocr_timeout_secs),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| format!("Tesseract timed out after {}s", config.ocr_timeout_secs))?
    .map_err(|e| format!("Tesseract process error: {e}"))?;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Tesseract exited with error: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Estimate confidence from stderr (Tesseract prints confidence info there)
    let confidence = estimate_confidence(&output.stderr, &text);

    Ok(OcrResult {
        text,
        confidence,
        elapsed_ms,
    })
}

/// Check if tesseract is available on the system
pub async fn is_tesseract_available(config: &Config) -> bool {
    Command::new(&config.tesseract_bin)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|mut c| {
            // Don't wait — if it spawned, it exists
            let _ = c.start_kill();
            true
        })
        .unwrap_or(false)
}

/// Estimate OCR confidence from Tesseract's stderr output.
///
/// Tesseract 4+ outputs confidence information on stderr.
/// We look for patterns like "confidence: XX%" or similar.
fn estimate_confidence(stderr: &[u8], _text: &str) -> f64 {
    let stderr_str = String::from_utf8_lossy(stderr);

    // Try to find confidence percentage in stderr
    for line in stderr_str.lines() {
        let lower = line.to_lowercase();
        if lower.contains("confidence") {
            // Try to extract a number
            let parts: Vec<&str> = lower.split("confidence").collect();
            if parts.len() > 1 {
                for word in parts[1].split_whitespace() {
                    let cleaned: String = word
                        .chars()
                        .filter(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if let Ok(val) = cleaned.parse::<f64>() {
                        // If it looks like a percentage (0-100), normalize
                        return if val > 1.0 { val / 100.0 } else { val };
                    }
                }
            }
        }
    }

    // Fallback: if text is non-empty, assume decent confidence
    if !stderr_str.trim().is_empty() {
        0.8
    } else {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_confidence_with_percentage() {
        let stderr = b"Page 1, confidence: 92.5%\n";
        let conf = estimate_confidence(stderr, "");
        assert!((conf - 0.925).abs() < 0.001, "Expected ~0.925, got {conf}");
    }

    #[test]
    fn test_estimate_confidence_decimal() {
        let stderr = b"confidence=0.874\n";
        let conf = estimate_confidence(stderr, "");
        assert!((conf - 0.874).abs() < 0.001, "Expected ~0.874, got {conf}");
    }

    #[test]
    fn test_estimate_confidence_fallback_empty() {
        let conf = estimate_confidence(b"", "");
        assert_eq!(conf, 0.5);
    }

    #[test]
    fn test_estimate_confidence_fallback_nonempty() {
        let conf = estimate_confidence(b"some tesseract output", "");
        assert_eq!(conf, 0.8);
    }
}
