//! 文档栏共享的展示信息，不持有文档或输入焦点。
use super::document::{Document, Encoding, Newline};
use super::editor;

pub(super) fn location(document: &Document) -> String {
    document.path.as_ref().map_or_else(
        || "未保存的文档".to_owned(),
        |path| path.display().to_string(),
    )
}

pub(super) fn metadata(document: &Document) -> String {
    let encoding = match document.encoding {
        Encoding::Utf8 => "UTF-8",
        Encoding::Utf8Bom => "UTF-8 BOM",
    };
    let newline = match document.newline {
        Newline::Lf => "LF",
        Newline::Crlf => "CRLF",
    };
    format!(
        "空格: {}   ·   {}   ·   {}   ·   {}",
        document.settings.tab_size,
        encoding,
        newline,
        editor::language_label(document.language.as_ref())
    )
}

pub(super) fn file_mark(language: &str) -> &'static str {
    match language {
        "markdown" => "MD",
        "plaintext" => "TXT",
        "yaml" | "toml" => "≡",
        _ => "{}",
    }
}
