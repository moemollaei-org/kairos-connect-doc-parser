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
