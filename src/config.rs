#[derive(Clone)]
pub struct Config {
    pub api_key: String,
    pub port: u16,
    pub max_concurrent: usize,
    pub body_limit_bytes: usize,
    /// OCR language codes (comma-separated, e.g. "eng", "eng+spa")
    pub ocr_languages: String,
    /// DPI for PDF page rendering before OCR (default 300)
    pub ocr_dpi: u32,
    /// Tesseract CLI binary path (default "tesseract")
    pub tesseract_bin: String,
    /// pdftoppm CLI binary path (default "pdftoppm")
    pub pdftoppm_bin: String,
    /// Timeout in seconds for OCR operations (default 60)
    pub ocr_timeout_secs: u64,
    /// `pdfimages` CLI path (poppler). Used for the scanned-PDF fast path.
    pub pdfimages_bin: String,
    /// Longest edge, in pixels, fed to Tesseract. Scans are commonly embedded at
    /// 600 PPI (~5100x7016); recognising at that size costs several seconds per
    /// page for no accuracy gain. 3400 measured as the point where word recall
    /// stops improving.
    pub ocr_max_pixels: u32,
    /// How many pages to OCR concurrently. Defaults to the core count.
    pub ocr_page_concurrency: usize,
}

impl Config {
    pub fn from_env() -> Self {
        let body_limit = std::env::var("DOC_PARSER_MAX_BODY_SIZE")
            .ok()
            .and_then(|s| parse_size(&s))
            .unwrap_or(200 * 1024 * 1024); // 200 MB default

        Self {
            api_key: std::env::var("DOC_PARSER_API_KEY")
                .unwrap_or_else(|_| "change-me-in-railway-dashboard".into()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".into())
                .parse()
                .expect("PORT must be a valid u16"),
            max_concurrent: std::env::var("DOC_PARSER_MAX_CONCURRENT")
                .unwrap_or_else(|_| "16".into())
                .parse()
                .expect("DOC_PARSER_MAX_CONCURRENT must be a valid usize"),
            body_limit_bytes: body_limit,
            ocr_languages: std::env::var("DOC_PARSER_OCR_LANGUAGES")
                .unwrap_or_else(|_| "eng".into()),
            ocr_dpi: std::env::var("DOC_PARSER_OCR_DPI")
                .unwrap_or_else(|_| "300".into())
                .parse()
                .expect("DOC_PARSER_OCR_DPI must be a valid u32"),
            tesseract_bin: std::env::var("DOC_PARSER_TESSERACT_BIN")
                .unwrap_or_else(|_| "tesseract".into()),
            pdftoppm_bin: std::env::var("DOC_PARSER_PDFTOPPM_BIN")
                .unwrap_or_else(|_| "pdftoppm".into()),
            ocr_timeout_secs: std::env::var("DOC_PARSER_OCR_TIMEOUT_SECS")
                .unwrap_or_else(|_| "60".into())
                .parse()
                .expect("DOC_PARSER_OCR_TIMEOUT_SECS must be a valid u64"),
            pdfimages_bin: std::env::var("DOC_PARSER_PDFIMAGES_BIN")
                .unwrap_or_else(|_| "pdfimages".into()),
            ocr_max_pixels: std::env::var("DOC_PARSER_OCR_MAX_PIXELS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3400),
            ocr_page_concurrency: std::env::var("DOC_PARSER_OCR_PAGE_CONCURRENCY")
                .ok()
                .and_then(|s| s.parse().ok())
                .filter(|n| *n > 0)
                .unwrap_or_else(|| {
                    std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(2)
                }),
        }
    }
}

/// Parse a size string like "200MB", "200mb", "1GB", "209715200"
fn parse_size(s: &str) -> Option<usize> {
    let s = s.trim();
    if let Ok(n) = s.parse::<usize>() {
        return Some(n);
    }
    let s_lower = s.to_lowercase();
    let (num_str, mult) = if s_lower.ends_with("gb") {
        (&s[..s.len() - 2], 1024 * 1024 * 1024)
    } else if s_lower.ends_with("mb") {
        (&s[..s.len() - 2], 1024 * 1024)
    } else if s_lower.ends_with("kb") {
        (&s[..s.len() - 2], 1024)
    } else {
        return None;
    };
    num_str.trim().parse::<usize>().ok().map(|n| n * mult)
}
