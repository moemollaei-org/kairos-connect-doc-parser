use std::time::Instant;

use crate::models::ConvertResult;

pub struct ConvertInput {
    pub index: usize,
    pub filename: String,
    pub bytes: Vec<u8>,
    pub format_hint: Option<anydoc::Format>,
}

/// CPU-bound conversion, runs in spawn_blocking
pub async fn convert_one(input: ConvertInput) -> ConvertResult {
    let filename = input.filename;
    let index = input.index;
    let start = Instant::now();

    let result = tokio::task::spawn_blocking(move || {
        let hint: Option<anydoc::Format> = input.format_hint;
        let detected = anydoc::Format::from_bytes(&input.bytes);
        anydoc::to_markdown_bytes(&input.bytes, hint.or(detected))
    })
    .await;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(md)) => ConvertResult {
            index,
            filename,
            markdown: Some(md),
            error: None,
            elapsed_ms,
        },
        Ok(Err(e)) => ConvertResult {
            index,
            filename,
            markdown: None,
            error: Some(error_msg(&e)),
            elapsed_ms,
        },
        Err(je) => ConvertResult {
            index,
            filename,
            markdown: None,
            error: Some(format!("Conversion task panicked: {je}")),
            elapsed_ms,
        },
    }
}

fn error_msg(e: &anydoc::ConvertError) -> String {
    match e {
        anydoc::ConvertError::Encrypted => "Document is encrypted or password-protected".into(),
        anydoc::ConvertError::Unsupported(_) => "Unsupported document format".into(),
        anydoc::ConvertError::Malformed { .. } => "Document is malformed or corrupt".into(),
        anydoc::ConvertError::ResourceLimit { .. } => "Document exceeds processing limits".into(),
        anydoc::ConvertError::MissingPart { .. } => "Required part of the document is missing".into(),
        anydoc::ConvertError::Io(io) => format!("Could not read document: {io}"),
        _ => format!("Conversion error: {e:?}"),
    }
}
