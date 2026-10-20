//! 编辑器共享能力：操作类型、语言识别和格式化工具。

use gpui_kit::actions;

actions!(
    text_editor,
    [
        New,
        Open,
        Save,
        SaveAs,
        Format,
        Close,
        SetPlaintext,
        SetRust,
        SetPython,
        SetJavaScript,
        SetTypeScript,
        SetJson,
        SetHtml,
        SetCss,
        SetMarkdown,
        SetToml,
        SetYaml,
        ToggleFolding,
        FindNext,
        FindPrevious
    ]
);

pub(super) const LANGUAGE_OPTIONS: &[(&str, &str)] = &[
    ("plaintext", "纯文本"),
    ("rust", "Rust"),
    ("python", "Python"),
    ("javascript", "JavaScript"),
    ("typescript", "TypeScript"),
    ("json", "JSON"),
    ("html", "HTML"),
    ("css", "CSS"),
    ("markdown", "Markdown"),
    ("toml", "TOML"),
    ("yaml", "YAML"),
];

pub(super) fn language_label(language: &str) -> &'static str {
    LANGUAGE_OPTIONS
        .iter()
        .find_map(|(value, label)| (*value == language).then_some(*label))
        .unwrap_or("纯文本")
}

/// 根据文件扩展名选择 Tree-sitter 语言；未知类型回退到纯文本。
pub(super) fn language_for_path(path: &std::path::Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "javascript",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "sh" | "bash" => "bash",
        "sql" => "sql",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "vue" => "html",
        "md" | "markdown" | "mdown" => "markdown",
        "json" | "jsonl" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "html",
        "csv" | "tsv" | "txt" | "text" => "plaintext",
        _ => "plaintext",
    }
}

/// 判断路径是否应当按 JSON 语法处理。
pub(super) fn is_json_path(path: Option<&std::path::Path>) -> bool {
    path.and_then(|path| path.extension())
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(extension.to_ascii_lowercase().as_str(), "json" | "jsonl")
        })
}

/// JSON 格式化的结果，同时标记是否经过了容错修复。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum JsonFormatResult {
    Formatted(String),
    Repaired(String),
}

/// 严格解析 JSON；失败后使用 `jsonrepair` 修复，再将修复结果严格解析并美化输出。
pub(super) fn format_json(value: &str) -> Result<JsonFormatResult, String> {
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(json) => serde_json::to_string_pretty(&json)
            .map(|formatted| JsonFormatResult::Formatted(format!("{formatted}\n")))
            .map_err(|error| format!("JSON 格式化失败：{error}")),
        Err(strict_error) => {
            let repaired = jsonrepair::repair_to_string(value, &jsonrepair::Options::default())
                .map_err(|repair_error| {
                    format!("JSON 解析失败：{strict_error}；自动修复失败：{repair_error}")
                })?;
            let json = serde_json::from_str::<serde_json::Value>(&repaired)
                .map_err(|error| format!("JSON 自动修复结果仍无效：{error}"))?;
            serde_json::to_string_pretty(&json)
                .map(|formatted| JsonFormatResult::Repaired(format!("{formatted}\n")))
                .map_err(|error| format!("JSON 修复后格式化失败：{error}"))
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
