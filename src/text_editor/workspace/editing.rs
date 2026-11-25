// 文本转换、格式化与光标跳转。
use super::super::{document::DocumentId, editor};
use super::Workspace;
use gpui_kit::*;

impl Workspace {
    /// 把文本命令做成一次 `replace_all`。内容没变时不写入，避免空的撤销记录。
    fn apply_text_command(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
        operation: impl FnOnce(
            &str,
            std::ops::Range<usize>,
            usize,
        ) -> Result<
            (String, Option<std::ops::Range<usize>>),
            editor::TextCommandError,
        >,
    ) {
        let Some(document) = self.document(id) else {
            return;
        };
        let editor_state = document.editor.clone();
        let state = editor_state.read(cx);
        let value = state.value().to_string();
        let selection = state.selected_range();
        let cursor_position = state.cursor_position();
        let cursor = editor::offset_for_position(
            &value,
            cursor_position.line as usize,
            cursor_position.character as usize,
        );
        let Ok((updated, restored_selection)) = operation(&value, selection.clone(), cursor) else {
            self.message = "当前命令没有可处理的内容".into();
            cx.notify();
            return;
        };
        if updated == value {
            return;
        }
        editor_state.update(cx, |state, cx| {
            state.replace_all(updated.clone(), window, cx);
            // 行操作不返回新选区，按命令前光标所在的行列恢复位置。
            if let Some(range) = restored_selection {
                state.set_selected_range(range, cx);
            } else {
                let position = editor::position_for_offset(&value, cursor);
                state.set_cursor_position(
                    component::input::Position::new(position.0 as u32, position.1 as u32),
                    window,
                    cx,
                );
            }
        });
        self.message = "文本已更新".into();
        cx.notify();
    }

    /// 转换当前选择，并把选区扩到转换后的新长度。
    pub(super) fn transform_selection(
        &mut self,
        id: DocumentId,
        operation: editor::TextTransform,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_text_command(id, window, cx, move |text, range, _| {
            let result = editor::transform_selection(text, range.clone(), operation)?;
            let transformed =
                editor::transform_selection(&text[range.clone()], 0..range.len(), operation)?;
            Ok((result, Some(range.start..range.start + transformed.len())))
        });
    }

    /// 对光标所在行执行删除、移动或缩进，并作为一次可撤销编辑提交。
    pub(super) fn transform_line(
        &mut self,
        id: DocumentId,
        operation: editor::LineTransform,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_text_command(id, window, cx, move |text, _, cursor| {
            Ok((
                editor::transform_current_line(text, cursor, operation)?,
                None,
            ))
        });
    }

    /// 在当前行后面插入一份副本。最后一行没有换行时先补一个 LF。
    pub(super) fn duplicate_current_line(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_text_command(id, window, cx, |text, _, cursor| {
            let ranges = editor::line_ranges(text);
            let index = ranges
                .iter()
                .position(|range| cursor >= range.start && cursor <= range.end)
                .unwrap_or_else(|| ranges.len().saturating_sub(1));
            let line = &ranges[index];
            // 行范围不含换行符，复制时要把行尾的 LF 一起算进去。
            let has_newline = text.as_bytes().get(line.end) == Some(&b'\n');
            let end = line.end + usize::from(has_newline);
            let mut result = String::with_capacity(text.len() + line.len() + 1);
            result.push_str(&text[..end]);
            if !has_newline {
                result.push('\n');
            }
            result.push_str(&text[line.clone()]);
            if has_newline {
                result.push('\n');
            }
            result.push_str(&text[end..]);
            Ok((result, None))
        });
    }

    /// 清理行尾空白，并把原选区映射到清理后的文本上。
    pub(super) fn trim_trailing_whitespace(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_text_command(id, window, cx, |text, selection, _| {
            let updated = editor::trim_trailing_whitespace(text);
            let selection = editor::remap_range_after_trimming(text, selection)?;
            Ok((updated, Some(selection)))
        });
    }

    /// 按 `行` 或 `行:列` 跳转。行号、列号都从 1 开始，列按字符而不是字节计数。
    pub(super) fn goto_line(
        &mut self,
        id: DocumentId,
        input: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(document) = self.document(id) else {
            return false;
        };
        let editor = document.editor.clone();
        let value = editor.read(cx).value().to_string();
        let lines: Vec<&str> = value.split('\n').collect();
        let line_count = lines.len();
        // 先用全文最长行做列上限，真正落点还要再对目标行检查一次。
        let max_column = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
            .max(1);
        match editor::parse_goto_position(input, line_count, max_column) {
            Ok((line, character)) => {
                if character > lines[line].chars().count() {
                    self.message = "列号超出范围".into();
                    cx.notify();
                    return false;
                }
                editor.update(cx, |state, cx| {
                    state.set_cursor_position(
                        component::input::Position::new(line as u32, character as u32),
                        window,
                        cx,
                    )
                });
                self.message = "已跳转".into();
                cx.notify();
                true
            }
            Err(error) => {
                self.message = match error {
                    editor::TextCommandError::LineOutOfRange => "行号超出范围",
                    _ => "请输入有效的行号",
                }
                .into();
                cx.notify();
                false
            }
        }
    }

    /// 格式化当前文档。JSON 走严格解析和容错修复，其他文本只清理行尾空白。
    pub(super) fn format_document(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_busy() {
            return;
        }
        let Some(document) = self.document(id) else {
            return;
        };
        let value = document.editor.read(cx).value().to_string();
        let path = document.path.clone();
        let (formatted, status) = if editor::is_json_path(path.as_deref()) {
            match editor::format_json(&value) {
                Ok(editor::JsonFormatResult::Formatted(formatted)) => (formatted, "JSON 已格式化"),
                Ok(editor::JsonFormatResult::Repaired(formatted)) => {
                    (formatted, "JSON 已修复并格式化")
                }
                Err(error) => {
                    self.message = error.into();
                    cx.notify();
                    return;
                }
            }
        } else {
            // 非 JSON 只去掉行尾空白，并在非空文本末尾补一个换行。
            let mut formatted = value
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n");
            if !formatted.is_empty() {
                formatted.push('\n');
            }
            (formatted, "已格式化")
        };
        if formatted == value {
            self.message = "无需格式化".into();
            cx.notify();
            return;
        }
        if let Some(document) = self.document_mut(id) {
            document
                .editor
                .update(cx, |state, cx| state.replace_all(formatted, window, cx));
        }
        self.message = status.into();
        cx.notify();
    }
}
