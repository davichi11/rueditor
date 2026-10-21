//! 编辑器共享能力：操作类型、语言识别和格式化工具。

use super::document::{SearchMode, SearchOptions};
use gpui_kit::actions;
use regex::{Regex, RegexBuilder};
use std::ops::Range;
use std::sync::OnceLock;

// 菜单、快捷键和工作区监听共用这一组动作类型。
actions!(
    text_editor,
    [
        New,
        Open,
        Save,
        SaveAs,
        Format,
        Close,
        Quit,
        SetTsx,
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
        ToggleLineNumbers,
        ToggleSoftWrap,
        ToggleShowWhitespaces,
        ToggleSmartIndent,
        ToggleAutoClose,
        SetTabSize2,
        SetTabSize4,
        SetTabSize8,
        FindNext,
        FindPrevious,
        OpenSearchPanel,
        ToggleRegex,
        ToggleCaseSensitive,
        ToggleWholeWord,
        ReplaceCurrent,
        ReplaceAllMatches,
        CloseSearchPanel,
        GotoLine,
        DuplicateLine,
        DeleteLine,
        MoveLineUp,
        MoveLineDown,
        IndentSelection,
        OutdentSelection,
        UppercaseSelection,
        LowercaseSelection,
        TrimTrailingWhitespace,
        ShowAbout
    ]
);

/// 语言标识和状态栏上的显示名。
pub(super) const LANGUAGE_OPTIONS: &[(&str, &str)] = &[
    ("plaintext", "纯文本"),
    ("rust", "Rust"),
    ("python", "Python"),
    ("javascript", "JavaScript"),
    ("typescript", "TypeScript"),
    ("tsx", "TSX"),
    ("json", "JSON"),
    ("html", "HTML"),
    ("css", "CSS"),
    ("markdown", "Markdown"),
    ("toml", "TOML"),
    ("yaml", "YAML"),
];

/// 搜索输入或匹配过程中的可恢复错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SearchError {
    /// 查询为空，不启动匹配。
    EmptyQuery,
    /// 正则无法编译。`reason` 来自正则引擎。
    InvalidPattern { query: String, reason: String },
    /// 表达式有效，但文本里没有符合选项的匹配。
    NoMatches { query: String },
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyQuery => formatter.write_str("搜索内容不能为空"),
            Self::InvalidPattern { query, reason } => {
                write!(formatter, "正则表达式 {query:?} 无效：{reason}")
            }
            Self::NoMatches { query } => write!(formatter, "没有找到匹配项：{query:?}"),
        }
    }
}

impl std::error::Error for SearchError {}

/// 在文本中查找匹配项，返回 UTF-8 byte ranges。
pub(super) fn find_matches(
    text: &str,
    query: &str,
    options: SearchOptions,
) -> Result<Vec<Range<usize>>, SearchError> {
    let regex = compile_search_regex(query, options)?;
    Ok(regex
        .find_iter(text)
        .filter(|matched| {
            !options.whole_word || is_whole_word_match(text, matched.start(), matched.end())
        })
        .map(|matched| matched.start()..matched.end())
        .collect())
}

/// 替换所有匹配项；失败时不产生部分替换结果。
pub(super) fn replace_matches(
    text: &str,
    query: &str,
    replacement: &str,
    options: SearchOptions,
) -> Result<ReplaceResult, SearchError> {
    let regex = compile_search_regex(query, options)?;
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;
    let mut count = 0;

    // 全词过滤发生在展开替换之前；被跳过的区间留在原文里。
    for captures in regex.captures_iter(text) {
        let matched = captures
            .get(0)
            .expect("regex captures always contain the full match");
        if options.whole_word && !is_whole_word_match(text, matched.start(), matched.end()) {
            continue;
        }

        result.push_str(&text[cursor..matched.start()]);
        match options.mode {
            SearchMode::PlainText => result.push_str(replacement),
            SearchMode::Regex => captures.expand(replacement, &mut result),
        }
        cursor = matched.end();
        count += 1;
    }

    if count == 0 {
        return Err(SearchError::NoMatches {
            query: query.to_owned(),
        });
    }
    result.push_str(&text[cursor..]);
    Ok(ReplaceResult {
        text: result,
        count,
    })
}

/// 全部替换的纯文本结果和实际替换数量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReplaceResult {
    pub(super) text: String,
    pub(super) count: usize,
}

/// 按选项编译查找表达式。普通模式先转义，避免把用户输入当成正则。
fn compile_search_regex(query: &str, options: SearchOptions) -> Result<Regex, SearchError> {
    if query.is_empty() {
        return Err(SearchError::EmptyQuery);
    }

    let pattern = match options.mode {
        SearchMode::PlainText => regex::escape(query),
        SearchMode::Regex => query.to_owned(),
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!options.case_sensitive)
        .build()
        .map_err(|error| SearchError::InvalidPattern {
            query: query.to_owned(),
            reason: error.to_string(),
        })
}

/// 匹配区间两侧都不是 Unicode 词字符时，才算全词匹配。
fn is_whole_word_match(text: &str, start: usize, end: usize) -> bool {
    let previous_is_word = text[..start]
        .chars()
        .next_back()
        .is_some_and(is_unicode_word_character);
    let next_is_word = text[end..]
        .chars()
        .next()
        .is_some_and(is_unicode_word_character);
    !previous_is_word && !next_is_word
}

/// 用 `\w` 的 Unicode 语义判断词字符，中文等非 ASCII 字符也算在内。
fn is_unicode_word_character(character: char) -> bool {
    static WORD_CHARACTER: OnceLock<Regex> = OnceLock::new();
    let regex = WORD_CHARACTER
        .get_or_init(|| Regex::new(r"\A\w\z").expect("the built-in Unicode word pattern is valid"));
    let mut encoded = [0; 4];
    regex.is_match(character.encode_utf8(&mut encoded))
}

/// 可作用于当前选择的纯文本转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TextTransform {
    /// 选中文本转为大写。Unicode 大小写转换可能改变字节长度。
    Uppercase,
    /// 选中文本转为小写。
    Lowercase,
}

/// 可作用于光标所在行的纯文本操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LineTransform {
    /// 删除光标所在行，并吃掉相邻的一个换行符。
    Delete,
    /// 与上一行交换。已经在第一行时文本不变。
    MoveUp,
    /// 与下一行交换。已经在最后一行时文本不变。
    MoveDown,
    /// 在行首插入四个空格。
    Indent,
    /// 去掉行首的一个制表符，或最多四个空格。
    Outdent,
}

/// 文本命令的可恢复输入与范围错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TextCommandError {
    /// 输入为空、含非数字，或 `行:列` 的格式不对。
    InvalidInput,
    /// 行号小于 1，或大于当前文本行数。
    LineOutOfRange,
    /// 列号小于 1，或大于允许的最大列。
    ColumnOutOfRange,
    /// 选区端点越界，或落在 UTF-8 字符中间。
    InvalidRange,
    /// 选区为空，没有可转换的文本。
    EmptySelection,
    /// 光标 offset 越界，或不在字符边界上。
    InvalidCursor,
}

impl std::fmt::Display for TextCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidInput => "请输入有效的位置",
            Self::LineOutOfRange => "行号超出范围",
            Self::ColumnOutOfRange => "列号超出范围",
            Self::InvalidRange => "选择范围无效",
            Self::EmptySelection => "没有可转换的选择内容",
            Self::InvalidCursor => "光标位置无效",
        };
        formatter.write_str(message)
    }
}

/// 解析用户输入的 1-based 行号和可选列号，返回 0-based 位置。
pub(super) fn parse_goto_position(
    input: &str,
    line_count: usize,
    max_column: usize,
) -> Result<(usize, usize), TextCommandError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(TextCommandError::InvalidInput);
    }

    let (line_input, column_input) = match input.split_once(':') {
        Some((line, column)) if !column.contains(':') => (line, Some(column)),
        Some(_) => return Err(TextCommandError::InvalidInput),
        None => (input, None),
    };
    let line = parse_positive_decimal(line_input)?;
    let column = column_input.map_or(Ok(1), parse_positive_decimal)?;

    if line > line_count || line_count == 0 {
        return Err(TextCommandError::LineOutOfRange);
    }
    if column > max_column || max_column == 0 {
        return Err(TextCommandError::ColumnOutOfRange);
    }
    Ok((line - 1, column - 1))
}

/// 解析正十进制整数。空串、符号和非数字字符都视为无效输入。
fn parse_positive_decimal(input: &str) -> Result<usize, TextCommandError> {
    if input.is_empty() || !input.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(TextCommandError::InvalidInput);
    }
    input
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or(TextCommandError::InvalidInput)
}

/// 对 UTF-8 byte range 中的非空选择执行大小写转换。
pub(super) fn transform_selection(
    text: &str,
    range: Range<usize>,
    operation: TextTransform,
) -> Result<String, TextCommandError> {
    if range.start > range.end || range.end > text.len() {
        return Err(TextCommandError::InvalidRange);
    }
    let selected = text
        .get(range.clone())
        .ok_or(TextCommandError::InvalidRange)?;
    if selected.is_empty() {
        return Err(TextCommandError::EmptySelection);
    }

    let transformed = match operation {
        TextTransform::Uppercase => selected.to_uppercase(),
        TextTransform::Lowercase => selected.to_lowercase(),
    };
    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..range.start]);
    result.push_str(&transformed);
    result.push_str(&text[range.end..]);
    Ok(result)
}

/// 对 UTF-8 byte offset 所在行执行行操作。
pub(super) fn transform_current_line(
    text: &str,
    cursor: usize,
    operation: LineTransform,
) -> Result<String, TextCommandError> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return Err(TextCommandError::InvalidCursor);
    }

    let lines = line_ranges(text);
    let index = lines
        .iter()
        .position(|range| cursor >= range.start && cursor <= range.end)
        .or_else(|| (cursor == text.len()).then_some(lines.len() - 1))
        .ok_or(TextCommandError::InvalidCursor)?;
    let line = lines[index].clone();

    match operation {
        LineTransform::Delete => {
            let mut result = String::with_capacity(text.len());
            // 中间行连同自己的换行符删掉；最后一行则改删它前面的换行符。
            if text.as_bytes().get(line.end) == Some(&b'\n') {
                result.push_str(&text[..line.start]);
                result.push_str(&text[line.end + 1..]);
            } else if line.start > 0 {
                result.push_str(&text[..line.start - 1]);
                result.push_str(&text[line.end..]);
            } else {
                result.push_str(&text[line.end..]);
            }
            Ok(result)
        }
        LineTransform::MoveUp | LineTransform::MoveDown => {
            let target = match operation {
                LineTransform::MoveUp if index > 0 => index - 1,
                LineTransform::MoveDown if index + 1 < lines.len() => index + 1,
                _ => return Ok(text.to_owned()),
            };
            // 先拆成不含换行的行再交换，最后按原文是否以换行结尾还原。
            let has_final_newline = text.ends_with('\n');
            let mut contents: Vec<&str> = text.split('\n').collect();
            if has_final_newline {
                contents.pop();
            }
            contents.swap(index, target);
            let mut result = contents.join("\n");
            if has_final_newline {
                result.push('\n');
            }
            Ok(result)
        }
        LineTransform::Indent => {
            let mut result = String::with_capacity(text.len() + 4);
            result.push_str(&text[..line.start]);
            result.push_str("    ");
            result.push_str(&text[line.start..]);
            Ok(result)
        }
        LineTransform::Outdent => {
            let line_text = &text[line.clone()];
            // 优先去掉一个制表符，否则最多去掉四个前导空格。
            let remove = if line_text.starts_with('\t') {
                1
            } else {
                line_text
                    .bytes()
                    .take_while(|byte| *byte == b' ')
                    .count()
                    .min(4)
            };
            if remove == 0 {
                return Ok(text.to_owned());
            }
            let mut result = String::with_capacity(text.len() - remove);
            result.push_str(&text[..line.start]);
            result.push_str(&text[line.start + remove..]);
            Ok(result)
        }
    }
}

/// 按 LF 切出每行内容的 UTF-8 byte range，不含换行符。空文本也返回一行。
pub(super) fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (offset, character) in text.char_indices() {
        if character == '\n' {
            ranges.push(start..offset);
            start = offset + 1;
        }
    }
    if start < text.len() || ranges.is_empty() {
        ranges.push(start..text.len());
    }
    ranges
}

/// 把 0-based 行列换成 UTF-8 byte offset。列超出该行时停在行尾。
pub(super) fn offset_for_position(text: &str, line: usize, character: usize) -> usize {
    let ranges = line_ranges(text);
    let Some(range) = ranges.get(line) else {
        return text.len();
    };
    let line_text = &text[range.clone()];
    range.start
        + line_text
            .char_indices()
            .nth(character)
            .map_or(line_text.len(), |(offset, _)| offset)
}

/// 清理每行末尾的 Unicode 空白并保留 LF 行结构。
pub(super) fn trim_trailing_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for segment in text.split_inclusive('\n') {
        if let Some(line) = segment.strip_suffix('\n') {
            result.push_str(line.trim_end_matches(char::is_whitespace));
            result.push('\n');
        } else {
            result.push_str(segment.trim_end_matches(char::is_whitespace));
        }
    }
    result
}

/// 将原文本中的 UTF-8 byte range 映射到清理行尾空白后的文本。
pub(super) fn remap_range_after_trimming(
    text: &str,
    range: Range<usize>,
) -> Result<Range<usize>, TextCommandError> {
    if range.start > range.end || range.end > text.len() {
        return Err(TextCommandError::InvalidRange);
    }
    if !text.is_char_boundary(range.start) || !text.is_char_boundary(range.end) {
        return Err(TextCommandError::InvalidRange);
    }

    let mut mapped_start = range.start;
    let mut mapped_end = range.end;

    let mut source_offset = 0;
    let mut target_offset = 0;
    for segment in text.split_inclusive('\n') {
        let content = segment.strip_suffix('\n').unwrap_or(segment);
        let kept_len = content.trim_end_matches(char::is_whitespace).len();
        let source_end = source_offset + content.len();
        // 落在被删掉的行尾空白上的端点，收回到这一行保留下来的末尾。
        if range.start >= source_offset && range.start <= source_end {
            mapped_start = target_offset + range.start.saturating_sub(source_offset).min(kept_len);
        }
        if range.end >= source_offset && range.end <= source_end {
            mapped_end = target_offset + range.end.saturating_sub(source_offset).min(kept_len);
        }
        source_offset += segment.len();
        target_offset += kept_len + usize::from(segment.ends_with('\n'));
    }
    Ok(mapped_start..mapped_end)
}

/// 把 UTF-8 byte offset 换成 0-based 行列。列按 Unicode 标量计数，不按字节。
pub(super) fn position_for_offset(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let offset = (0..=offset)
        .rev()
        .find(|candidate| text.is_char_boundary(*candidate))
        .unwrap_or(0);
    let prefix = &text[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    (line, prefix[line_start..].chars().count())
}

/// 返回语言的界面名称。未知标识显示为纯文本。
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
        "html" | "htm" => "html",
        "css" => "css",
        "md" | "markdown" | "mdown" => "markdown",
        "json" | "jsonl" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
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
    /// 原文已经是合法 JSON，只做了缩进。
    Formatted(String),
    /// 原文不合法，先修复再缩进。调用方应把文档标成已修改。
    Repaired(String),
}

/// 严格解析 JSON；失败后使用 `jsonrepair` 修复，再将修复结果严格解析并美化输出。
pub(super) fn format_json(value: &str) -> Result<JsonFormatResult, String> {
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(json) => serde_json::to_string_pretty(&json)
            .map(|formatted| JsonFormatResult::Formatted(format!("{formatted}\n")))
            .map_err(|error| format!("JSON 格式化失败：{error}")),
        Err(strict_error) => {
            // 修复结果还要再严格解析一次，避免把修坏的文本当成格式化成功。
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
