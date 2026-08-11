#[derive(Clone)]
pub struct Config {
    pub api_key: String,
    pub port: u16,
    pub max_concurrent: usize,
    pub body_limit_bytes: usize,
}

impl Config {
    pub fn from_env() -> Self {
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
            body_limit_bytes: 100 * 1024 * 1024,
        }
    }
}
