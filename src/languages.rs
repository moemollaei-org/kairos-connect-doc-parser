//! Tesseract language handling: discovery, validation, and script auto-detection.
//!
//! Language codes arrive from callers and end up as a `tesseract -l` argument,
//! so nothing is passed through unchecked: a request may only name languages
//! this container actually has installed.

use std::collections::BTreeSet;
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::process::Command;

use crate::config::Config;

static INSTALLED: OnceLock<BTreeSet<String>> = OnceLock::new();

/// Every language pack present in the image, from `tesseract --list-langs`.
///
/// Read once: the set cannot change while the process runs, and shelling out
/// per request would add a process spawn to every page.
pub async fn installed(config: &Config) -> &'static BTreeSet<String> {
    if let Some(set) = INSTALLED.get() {
        return set;
    }
    let set = discover(config).await.unwrap_or_default();
    INSTALLED.get_or_init(|| set)
}

async fn discover(config: &Config) -> Option<BTreeSet<String>> {
    let output = Command::new(&config.tesseract_bin)
        .arg("--list-langs")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .ok()?;

    // The first line is a header ("List of available languages ..."), the rest
    // are codes. `osd` and `snum` are detection/number models, not languages a
    // caller should select for text.
    let text = String::from_utf8_lossy(&output.stdout);
    Some(
        text.lines()
            .skip(1)
            .map(str::trim)
            .filter(|l| !l.is_empty() && *l != "osd" && *l != "snum")
            .map(str::to_string)
            .collect(),
    )
}

/// Why a requested language spec was rejected.
#[derive(Debug)]
pub enum LangError {
    Malformed(String),
    NotInstalled { requested: String, available: usize },
}

impl std::fmt::Display for LangError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LangError::Malformed(s) => write!(
                f,
                "Invalid language code '{s}'. Use Tesseract codes joined by '+', e.g. 'nld' or 'nld+eng'."
            ),
            LangError::NotInstalled {
                requested,
                available,
            } => write!(
                f,
                "Language '{requested}' is not installed ({available} available). GET /languages lists them."
            ),
        }
    }
}

/// Validate a caller-supplied spec such as `nld+eng` against installed packs.
///
/// Returns the spec unchanged when every component is known. Anything that is
/// not a plain code of the expected shape is rejected before it can reach the
/// command line.
pub async fn validate(config: &Config, spec: &str) -> Result<String, LangError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(LangError::Malformed(spec.to_string()));
    }
    let available = installed(config).await;

    for part in spec.split('+') {
        let part = part.trim();
        let shape_ok = !part.is_empty()
            && part.len() <= 16
            && part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !shape_ok {
            return Err(LangError::Malformed(part.to_string()));
        }
        if !available.contains(part) {
            return Err(LangError::NotInstalled {
                requested: part.to_string(),
                available: available.len(),
            });
        }
    }
    Ok(spec.to_string())
}

/// Languages worth trying for a script that Tesseract's OSD model reported.
///
/// OSD identifies the *script* (Han, Cyrillic, ...), not the language, and one
/// script serves many languages. Each script therefore maps to a small bundle:
/// broad enough to read the page, narrow enough that recognition stays fast.
fn languages_for_script(script: &str) -> &'static [&'static str] {
    match script {
        "Han" | "HanS" | "HanS_vert" => &["chi_sim", "chi_tra", "eng"],
        "HanT" | "HanT_vert" => &["chi_tra", "chi_sim", "eng"],
        "Japanese" | "Japanese_vert" => &["jpn", "eng"],
        "Korean" | "Korean_vert" => &["kor", "eng"],
        "Cyrillic" => &["rus", "ukr", "bul", "srp", "eng"],
        "Arabic" => &["ara", "fas", "urd", "eng"],
        "Hebrew" => &["heb", "eng"],
        "Greek" => &["ell", "eng"],
        "Devanagari" => &["hin", "mar", "nep", "san", "eng"],
        "Bengali" => &["ben", "eng"],
        "Tamil" => &["tam", "eng"],
        "Telugu" => &["tel", "eng"],
        "Kannada" => &["kan", "eng"],
        "Thai" => &["tha", "eng"],
        "Georgian" => &["kat", "eng"],
        "Armenian" => &["hye", "eng"],
        "Ethiopic" => &["amh", "tir", "eng"],
        "Myanmar" => &["mya", "eng"],
        "Khmer" => &["khm", "eng"],
        "Sinhala" => &["sin", "eng"],
        "Vietnamese" => &["vie", "eng"],
        // Latin covers most European languages. Tesseract's Latin models are
        // largely interchangeable for clean print, so the configured default
        // is used rather than guessing between dozens of candidates.
        _ => &[],
    }
}

/// Detect the script on a page and return a language spec that can read it.
///
/// Falls back to the configured default when detection fails or the script is
/// Latin — the caller always gets something usable.
pub async fn detect(config: &Config, image_bytes: &[u8]) -> String {
    let Some(script) = detect_script(config, image_bytes).await else {
        return config.ocr_languages.clone();
    };

    let candidates = languages_for_script(&script);
    if candidates.is_empty() {
        return config.ocr_languages.clone();
    }

    let available = installed(config).await;
    let usable: Vec<&str> = candidates
        .iter()
        .copied()
        .filter(|c| available.contains(*c))
        .collect();

    if usable.is_empty() {
        tracing::warn!(
            "detected script '{script}' but none of its language packs are installed; \
             falling back to {}",
            config.ocr_languages
        );
        return config.ocr_languages.clone();
    }
    tracing::debug!("detected script '{script}' -> {}", usable.join("+"));
    usable.join("+")
}

/// Ask Tesseract's orientation-and-script model what script a page uses.
async fn detect_script(config: &Config, image_bytes: &[u8]) -> Option<String> {
    let mut child = Command::new(&config.tesseract_bin)
        .args(["stdin", "stdout", "--psm", "0", "-l", "osd"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(image_bytes).await.ok()?;
        drop(stdin);
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(config.ocr_timeout_secs),
        child.wait_with_output(),
    )
    .await
    .ok()?
    .ok()?;

    // OSD prints lines like "Script: Han".
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Script: ").map(|s| s.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_falls_back_to_the_configured_default() {
        assert!(languages_for_script("Latin").is_empty());
        assert!(languages_for_script("Unknown").is_empty());
    }

    #[test]
    fn non_latin_scripts_map_to_their_languages() {
        assert!(languages_for_script("Cyrillic").contains(&"rus"));
        assert!(languages_for_script("Arabic").contains(&"ara"));
        assert!(languages_for_script("Japanese").contains(&"jpn"));
        assert!(languages_for_script("Han").contains(&"chi_sim"));
    }

    #[test]
    fn every_script_bundle_includes_english_as_a_fallback() {
        for script in ["Han", "Cyrillic", "Arabic", "Thai", "Devanagari"] {
            assert!(
                languages_for_script(script).contains(&"eng"),
                "{script} bundle should keep eng for embedded latin text"
            );
        }
    }
}
