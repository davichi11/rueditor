//! 多文档工作区：保留每个标签自己的编辑实体和文档元数据。

#[cfg(test)]
use super::document::SearchSession;
use super::document::{
    Document, DocumentId, EditorSettings, Encoding, Newline, SearchMode, SearchOptions,
};
use super::{editor, file_io};
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable, StyledExt, WindowExt as _,
    button::{Button, ButtonVariants},
    input::{Editor, EditorState, Escape, Input, InputEvent, InputState, Replace, Search},
    resizable::{h_resizable, resizable_panel},
    status_bar::StatusBar,
    tab::{Tab, TabBar},
    text::TextView,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use editor::{
    Close, CloseSearchPanel, DeleteLine, DuplicateLine, FindNext, FindPrevious, Format, GotoLine,
    IndentSelection, LowercaseSelection, MoveLineDown, MoveLineUp, New, Open, OpenSearchPanel,
    OutdentSelection, Quit, ReplaceAllMatches, ReplaceCurrent, Save, SaveAs, SetCss, SetHtml,
    SetJavaScript, SetJson, SetMarkdown, SetPlaintext, SetPython, SetRust, SetTabSize2,
    SetTabSize4, SetTabSize8, SetToml, SetTsx, SetTypeScript, SetYaml, ShowAbout, ToggleAutoClose,
    ToggleCaseSensitive, ToggleFolding, ToggleLineNumbers, TogglePreview, ToggleRegex,
    ToggleShowWhitespaces, ToggleSmartIndent, ToggleSoftWrap, ToggleWholeWord,
    TrimTrailingWhitespace, UppercaseSelection,
};

/// 一次只改文档设置里的一项，避免切换时把其他项重置成默认值。
#[derive(Clone, Copy)]
enum SettingChange {
    LineNumbers(bool),
    SoftWrap(bool),
    Whitespaces(bool),
    SmartIndent(bool),
    AutoClose(bool),
    TabSize(usize),
    Preview(bool),
}

/// 把文档设置整份写进编辑器实体。切换标签时用来恢复该文档自己的显示。
fn apply_editor_settings(
    state: &mut EditorState,
    settings: &EditorSettings,
    window: &mut Window,
    cx: &mut Context<EditorState>,
) {
    state.set_line_number(settings.show_line_numbers, window, cx);
    state.set_soft_wrap(settings.soft_wrap, window, cx);
    state.set_show_whitespaces(settings.show_whitespaces, window, cx);
    state.set_smart_indent(settings.smart_indent, window, cx);
    state.set_auto_close(settings.auto_close, window, cx);
    state.set_tab_size(
        base::input::TabSize {
            tab_size: settings.tab_size,
            hard_tabs: false,
        },
        cx,
    );
}

/// 保存成功且文档不再 dirty 之后要继续做的事。
#[derive(Clone, Copy)]
enum AfterSave {
    /// 保存的就是要关闭的那份文档时，接着移除标签。
    Close(DocumentId),
    /// 保存成功后继续询问下一个未保存文档。
    Quit,
}

/// 主窗口中的多文档状态。
pub(super) struct Workspace {
    documents: Vec<Document>,
    active_document_id: DocumentId,
    /// 下一个要分配的文档 ID，从 1 开始。0 只留给工作区级订阅。
    next_document_id: u64,
    focus: FocusHandle,
    /// 每次打开或拖入文件时加一，用来丢弃已经过期的加载结果。
    loading_request_id: u64,
    pending_loads: usize,
    loading: bool,
    /// 保存确认对话框还没返回时为 true。
    prompting: bool,
    /// 正在写盘的文档。同一文档不会并发保存。
    saving: HashSet<DocumentId>,
    quitting: bool,
    /// 这次退出里用户选择放弃的文档，避免对同一标签重复询问。
    quit_discarded: HashSet<DocumentId>,
    message: SharedString,
    /// 所有文档共用的查找输入框，内容从当前文档的搜索会话恢复。
    search_query: Entity<InputState>,
    search_replacement: Entity<InputState>,
    /// 程序回写输入框时为 true，用来忽略由此产生的 Change 事件。
    search_inputs_syncing: bool,
    /// 文档 ID 为 0 的条目是工作区输入框订阅，其余条目跟对应文档一起移除。
    _subscriptions: Vec<(DocumentId, Subscription)>,
}

impl Workspace {
    /// 创建包含一个未命名文档的新工作区。
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let search_query = cx.new(|cx| InputState::new(window, cx));
        let search_replacement = cx.new(|cx| InputState::new(window, cx));
        let mut workspace = Self {
            documents: Vec::new(),
            active_document_id: DocumentId(0),
            next_document_id: 1,
            focus,
            loading_request_id: 0,
            pending_loads: 0,
            loading: false,
            prompting: false,
            saving: HashSet::new(),
            quitting: false,
            quit_discarded: HashSet::new(),
            message: "".into(),
            search_query: search_query.clone(),
            search_replacement: search_replacement.clone(),
            search_inputs_syncing: false,
            _subscriptions: Vec::new(),
        };
        // DocumentId(0) 只标记工作区级输入订阅，不对应任何文档。
        workspace._subscriptions.push((
            DocumentId(0),
            cx.subscribe(&search_query, |this, input, event, cx| {
                // 切换文档时的回写也会发出 Change，不能再写回会话。
                if matches!(event, InputEvent::Change) && !this.search_inputs_syncing {
                    let value = input.read(cx).value().to_string();
                    let id = this.active_document_id;
                    if let Some(document) = this.document_mut(id) {
                        document.search.query = value;
                        document.search.error = None;
                    }
                    this.refresh_search(id, cx);
                }
            }),
        ));
        workspace._subscriptions.push((
            DocumentId(0),
            cx.subscribe(&search_replacement, |this, input, event, cx| {
                if matches!(event, InputEvent::Change) && !this.search_inputs_syncing {
                    let value = input.read(cx).value().to_string();
                    let id = this.active_document_id;
                    if let Some(document) = this.document_mut(id) {
                        document.search.replacement = value;
                    }
                    cx.notify();
                }
            }),
        ));
        let document =
            workspace.create_document(None, String::new(), Encoding::Utf8, Newline::Lf, window, cx);
        workspace.active_document_id = document.id;
        workspace.documents.push(document);
        workspace
    }

    /// 分配新 ID 并创建空白或带初始文本的文档，同时挂上修订监听。
    fn create_document(
        &mut self,
        path: Option<PathBuf>,
        text: String,
        encoding: Encoding,
        newline: Newline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Document {
        let id = DocumentId(self.next_document_id);
        self.next_document_id += 1;
        let language = path
            .as_deref()
            .map(editor::language_for_path)
            .unwrap_or("plaintext");
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(language)
                .folding(true)
                .default_value(text)
        });
        let settings = EditorSettings::default();
        editor.update(cx, |state, cx| {
            apply_editor_settings(state, &settings, window, cx)
        });
        let observed_id = id;
        self._subscriptions.push((
            observed_id,
            cx.observe(&editor, move |this, _, cx| {
                if this
                    .documents
                    .iter()
                    .any(|document| document.id == observed_id)
                {
                    cx.notify();
                }
            }),
        ));
        let subscribed_id = id;
        self._subscriptions.push((
            subscribed_id,
            cx.subscribe(&editor, move |this, _, event, cx| {
                // 只有正文变化才推进修订，光标移动不会把文档标成未保存。
                if matches!(event, InputEvent::Change)
                    && let Some(document) = this
                    .documents
                    .iter_mut()
                    .find(|document| document.id == subscribed_id)
                {
                    document.revision += 1;
                    cx.notify();
                }
            }),
        ));
        editor.update(cx, |state, cx| state.focus(window, cx));
        let title = path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "未命名".to_string());
        Document {
            id,
            path,
            title: title.into(),
            editor,
            settings,
            search: super::document::SearchSession::default(),
            language: language.into(),
            folding_enabled: true,
            encoding,
            newline,
            revision: 0,
            saved_revision: 0,
            disk_fingerprint: None,
        }
    }

    /// 新建标签并将其激活。
    pub(super) fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) -> DocumentId {
        let document =
            self.create_document(None, String::new(), Encoding::Utf8, Newline::Lf, window, cx);
        let id = document.id;
        self.documents.push(document);
        self.active_document_id = id;
        cx.notify();
        id
    }

    /// 切换到指定文档；未知 ID 不改变当前选择。
    pub(super) fn activate_document(&mut self, id: DocumentId) -> bool {
        if self.documents.iter().any(|document| document.id == id) {
            self.active_document_id = id;
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub(crate) fn activate_document_for_test(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.activate_document_with_focus(id, window, cx)
    }

    /// 激活文档，并把共用查找框和编辑器焦点切到这份文档。
    fn activate_document_with_focus(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.activate_document(id) {
            return false;
        }
        self.sync_search_inputs(id, window, cx);
        self.focus_document(id, window, cx);
        true
    }

    /// 用指定文档的搜索会话覆盖共用输入框，期间不把这次写入回写到会话。
    fn sync_search_inputs(&mut self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
        let Some((query, replacement)) = self.document(id).map(|document| {
            (
                document.search.query.clone(),
                document.search.replacement.clone(),
            )
        }) else {
            return;
        };
        self.search_inputs_syncing = true;
        self.search_query
            .update(cx, |input, cx| input.set_value(query, window, cx));
        self.search_replacement
            .update(cx, |input, cx| input.set_value(replacement, window, cx));
        self.search_inputs_syncing = false;
    }

    /// 把焦点交给文档编辑器，并重新套用该文档自己的显示设置。
    fn focus_document(&self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((editor, settings)) = self
            .document(id)
            .map(|document| (document.editor.clone(), document.settings.clone()))
        {
            editor.update(cx, |state, cx| {
                apply_editor_settings(state, &settings, window, cx);
                state.focus(window, cx);
            });
        }
    }

    /// 当前文档的稳定 ID。
    #[cfg(test)]
    pub(super) fn active_document_id(&self) -> DocumentId {
        self.active_document_id
    }

    /// 按稳定 ID 查找文档。标签重排后下标会变，调用方不要缓存下标。
    fn document(&self, id: DocumentId) -> Option<&Document> {
        self.documents.iter().find(|document| document.id == id)
    }

    /// `document` 的可变版本。
    fn document_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.documents.iter_mut().find(|document| document.id == id)
    }

    /// 测试和工作区逻辑使用的文档文本读取入口。
    #[cfg(test)]
    pub(super) fn document_text(&self, id: DocumentId, cx: &App) -> Option<String> {
        self.document(id)
            .map(|document| document.editor.read(cx).value().to_string())
    }

    /// 查询文档是否有未保存修改。
    #[cfg(test)]
    pub(super) fn document_is_dirty(&self, id: DocumentId) -> bool {
        self.document(id).is_some_and(Document::is_dirty)
    }

    #[cfg(test)]
    pub(super) fn document_revision_for_test(&self, id: DocumentId) -> Option<u64> {
        self.document(id).map(|document| document.revision)
    }

    #[cfg(test)]
    pub(super) fn set_search_session_for_test(&mut self, id: DocumentId, session: SearchSession) {
        if let Some(document) = self.document_mut(id) {
            document.search = session;
        }
    }

    #[cfg(test)]
    pub(super) fn search_session_for_test(&self, id: DocumentId) -> Option<SearchSession> {
        self.document(id).map(|document| document.search.clone())
    }

    /// 测试用的文档编辑入口，模拟真实编辑后保持各文档修订独立。
    #[cfg(test)]
    pub(super) fn set_document_value(
        &mut self,
        id: DocumentId,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(document) = self.document_mut(id) {
            document
                .editor
                .update(cx, |state, cx| state.set_value(value, window, cx));
            document.revision += 1;
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_all_for_test(
        &mut self,
        id: DocumentId,
        query: &str,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(document) = self.document(id) {
            document.editor.update(cx, |state, cx| {
                state.set_search_query(query, true, cx);
                state.replace_all_search_matches(replacement, window, cx);
            });
        }
    }

    #[cfg(test)]
    pub(crate) fn undo_document_for_test(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(document) = self.document(id) {
            document
                .editor
                .update(cx, |state, cx| state.focus(window, cx));
            window.dispatch_action(Box::new(component::input::Undo), cx);
        }
    }

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
    fn transform_selection(
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
    fn transform_line(
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
    fn duplicate_current_line(
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
    fn trim_trailing_whitespace(
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

    #[cfg(test)]
    pub(crate) fn set_document_selection_for_test(
        &mut self,
        id: DocumentId,
        range: std::ops::Range<usize>,
        cx: &mut Context<Self>,
    ) {
        if let Some(document) = self.document(id) {
            document
                .editor
                .update(cx, |state, cx| state.set_selected_range(range, cx));
        }
    }

    #[cfg(test)]
    pub(crate) fn set_document_cursor_for_test(
        &mut self,
        id: DocumentId,
        offset: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(document) = self.document(id) {
            let text = document.editor.read(cx).value().to_string();
            let (line, character) = editor::position_for_offset(&text, offset);
            document.editor.update(cx, |state, cx| {
                state.set_cursor_position(
                    component::input::Position::new(line as u32, character as u32),
                    window,
                    cx,
                )
            });
        }
    }

    #[cfg(test)]
    pub(crate) fn goto_line_for_test(
        &mut self,
        id: DocumentId,
        input: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.goto_line(id, input, window, cx)
    }

    #[cfg(test)]
    pub(crate) fn document_cursor_for_test(&self, id: DocumentId, cx: &App) -> Option<(u32, u32)> {
        self.document(id).map(|document| {
            let position = document.editor.read(cx).cursor_position();
            (position.line, position.character)
        })
    }

    #[cfg(test)]
    pub(crate) fn message_for_test(&self) -> &str {
        &self.message
    }

    /// 测试用的预览开关查询入口。
    #[cfg(test)]
    pub(super) fn document_preview_enabled_for_test(&self, id: DocumentId) -> Option<bool> {
        self.document(id)
            .map(|document| document.settings.show_preview)
    }

    /// 测试用的预览开关入口。
    #[cfg(test)]
    pub(crate) fn toggle_preview_for_test(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_preview(id, window, cx);
    }

    /// 按 `行` 或 `行:列` 跳转。行号、列号都从 1 开始，列按字符而不是字节计数。
    fn goto_line(
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

    /// 更换高亮语言。文本和修订号保持不变。
    fn set_document_language(&mut self, id: DocumentId, language: &str, cx: &mut Context<Self>) {
        let Some(document) = self.document_mut(id) else {
            return;
        };
        if document.language == language {
            return;
        }
        document.language = language.into();
        document.editor.update(cx, |state, cx| {
            state.set_highlighter(language, cx);
        });
        cx.notify();
    }

    /// 更新一项文档设置并同步到编辑器。值没有变化时不通知界面。
    fn set_document_setting(
        &mut self,
        id: DocumentId,
        change: SettingChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(document) = self.document_mut(id) else {
            return;
        };
        let changed = match change {
            SettingChange::LineNumbers(value) => {
                if document.settings.show_line_numbers == value {
                    false
                } else {
                    document.settings.show_line_numbers = value;
                    true
                }
            }
            SettingChange::SoftWrap(value) => {
                if document.settings.soft_wrap == value {
                    false
                } else {
                    document.settings.soft_wrap = value;
                    true
                }
            }
            SettingChange::Whitespaces(value) => {
                if document.settings.show_whitespaces == value {
                    false
                } else {
                    document.settings.show_whitespaces = value;
                    true
                }
            }
            SettingChange::SmartIndent(value) => {
                if document.settings.smart_indent == value {
                    false
                } else {
                    document.settings.smart_indent = value;
                    true
                }
            }
            SettingChange::AutoClose(value) => {
                if document.settings.auto_close == value {
                    false
                } else {
                    document.settings.auto_close = value;
                    true
                }
            }
            SettingChange::TabSize(value) => {
                if document.settings.tab_size == value {
                    false
                } else {
                    document.settings.tab_size = value;
                    true
                }
            }
            SettingChange::Preview(value) => {
                if document.settings.show_preview == value {
                    false
                } else {
                    document.settings.show_preview = value;
                    true
                }
            }
        };
        if !changed {
            return;
        }
        let editor = document.editor.clone();
        editor.update(cx, |state, cx| match change {
            SettingChange::LineNumbers(value) => state.set_line_number(value, window, cx),
            SettingChange::SoftWrap(value) => state.set_soft_wrap(value, window, cx),
            SettingChange::Whitespaces(value) => state.set_show_whitespaces(value, window, cx),
            SettingChange::SmartIndent(value) => state.set_smart_indent(value, window, cx),
            SettingChange::AutoClose(value) => state.set_auto_close(value, window, cx),
            SettingChange::TabSize(value) => state.set_tab_size(
                base::input::TabSize {
                    tab_size: value,
                    hard_tabs: false,
                },
                cx,
            ),
            // 预览面板不改变编辑器本身的状态。
            SettingChange::Preview(_) => {}
        });
        cx.notify();
    }

    /// 读取当前布尔设置后取反，再交给 `set_document_setting`。
    fn toggle_document_setting(
        &mut self,
        id: DocumentId,
        change: fn(&EditorSettings) -> bool,
        make_change: fn(bool) -> SettingChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.document(id).map(|document| change(&document.settings)) else {
            return;
        };
        self.set_document_setting(id, make_change(!current), window, cx);
    }

    /// 切换 Markdown 预览面板。
    ///
    /// 预览只对 Markdown 文档有意义，其他语言会给出提示并且不打开面板。
    fn toggle_preview(&mut self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(document) = self.document(id) else {
            return;
        };
        if !editor::supports_preview(document.language.as_ref()) {
            self.message = "只有 Markdown 文档支持预览".into();
            cx.notify();
            return;
        }
        let enabled = !document.settings.show_preview;
        self.set_document_setting(id, SettingChange::Preview(enabled), window, cx);
        self.message = if enabled {
            "已打开预览".into()
        } else {
            "已关闭预览".into()
        };
        cx.notify();
    }

    /// 从当前位置向后查找下一个匹配，到末尾后绕回开头。
    fn find_next(&mut self, id: DocumentId, cx: &mut Context<Self>) {
        self.navigate_search(id, true, cx);
    }

    /// 从当前位置向前查找上一个匹配，到开头后绕回末尾。
    fn find_previous(&mut self, id: DocumentId, cx: &mut Context<Self>) {
        self.navigate_search(id, false, cx);
    }

    /// 按方向移动到下一个匹配。普通模式走编辑器内置搜索，正则模式按 byte range 循环。
    fn navigate_search(&mut self, id: DocumentId, forward: bool, cx: &mut Context<Self>) {
        let Some(document) = self.document(id) else {
            return;
        };
        let editor = document.editor.clone();
        let text = editor.read(cx).value().to_string();
        let selection = editor.read(cx).selected_range();
        let session = document.search.clone();
        let options = session.options;
        if options.mode == SearchMode::PlainText {
            editor.update(cx, |state, cx| {
                // 借用编辑器内置搜索做匹配，随即关掉它的浮层。
                state.open_search(session.replace_mode, cx);
                state.set_search_query(session.query.clone(), !options.case_sensitive, cx);
                state.close_search(cx);
                if forward {
                    let _ = state.next_search_match(cx);
                } else {
                    let _ = state.previous_search_match(cx);
                }
            });
            return;
        }
        let ranges = match editor::find_matches(&text, &session.query, options) {
            Ok(ranges) => ranges,
            Err(error) => {
                self.set_search_error(id, Some(error.to_string()), cx);
                return;
            }
        };
        if ranges.is_empty() {
            self.set_search_error(id, None, cx);
            return;
        }
        // 已有选区时，向后从选区末尾继续，向前从选区开头继续，避免停在当前匹配。
        let cursor = if selection.is_empty() {
            selection.start
        } else if forward {
            selection.end
        } else {
            selection.start
        };
        let found = if forward {
            ranges
                .iter()
                .find(|range| range.start >= cursor)
                .or_else(|| ranges.first())
        } else {
            ranges
                .iter()
                .rev()
                .find(|range| range.end <= cursor)
                .or_else(|| ranges.last())
        };
        if let Some(range) = found {
            editor.update(cx, |state, cx| state.set_selected_range(range.clone(), cx));
            self.set_search_error(id, None, cx);
        }
    }

    /// 只更新会话里的错误文案，不改文档文本和修订号。
    fn set_search_error(&mut self, id: DocumentId, error: Option<String>, cx: &mut Context<Self>) {
        if let Some(document) = self.document_mut(id) {
            document.search.error = error;
        }
        cx.notify();
    }

    /// 替换当前选区。正则模式下，选区必须恰好是一个完整匹配。
    fn replace_current(&mut self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(document) = self.document(id) else {
            return;
        };
        let session = document.search.clone();
        let editor_state = document.editor.clone();
        let text = editor_state.read(cx).value().to_string();
        if session.options.mode == SearchMode::PlainText {
            editor_state.update(cx, |state, cx| {
                state.open_search(true, cx);
                state.set_search_query(session.query.clone(), !session.options.case_sensitive, cx);
                state.close_search(cx);
                let _ = state.replace_current_search_match(&session.replacement, window, cx);
            });
            return;
        }
        let selection = editor_state.read(cx).selected_range();
        let Ok(current) = text
            .get(selection.clone())
            .ok_or(editor::SearchError::NoMatches {
                query: session.query.clone(),
            })
        else {
            self.set_search_error(id, Some("当前选择不是匹配项".to_string()), cx);
            return;
        };
        let Ok(replaced) = editor::replace_matches(
            current,
            &session.query,
            &session.replacement,
            session.options,
        ) else {
            self.set_search_error(id, Some("当前选择不是匹配项".to_string()), cx);
            return;
        };
        if replaced.count != 1 {
            self.set_search_error(id, Some("当前选择不是单个匹配项".to_string()), cx);
            return;
        }
        // 只改选区对应的那一段，再整篇一次写入，保持单次撤销。
        let mut updated = String::with_capacity(text.len());
        updated.push_str(&text[..selection.start]);
        updated.push_str(&replaced.text);
        updated.push_str(&text[selection.end..]);
        editor_state.update(cx, |state, cx| {
            state.replace_all(updated, window, cx);
        });
        self.set_search_error(id, None, cx);
    }

    /// 替换全部匹配。正则模式先算出完整新文本，失败时不改编辑器内容。
    fn replace_all_matches(&mut self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(document) = self.document(id) else {
            return;
        };
        let session = document.search.clone();
        let editor_state = document.editor.clone();
        if session.options.mode == SearchMode::PlainText {
            editor_state.update(cx, |state, cx| {
                state.open_search(true, cx);
                state.set_search_query(session.query.clone(), !session.options.case_sensitive, cx);
                state.close_search(cx);
                let _ = state.replace_all_search_matches(&session.replacement, window, cx);
            });
            return;
        }
        let text = editor_state.read(cx).value().to_string();
        match editor::replace_matches(&text, &session.query, &session.replacement, session.options)
        {
            Ok(result) => {
                editor_state.update(cx, |state, cx| state.replace_all(result.text, window, cx));
                self.set_search_error(id, None, cx);
            }
            Err(error) => self.set_search_error(id, Some(error.to_string()), cx),
        }
    }

    /// 打开当前文档的查找面板，并把焦点放进共用查询框。
    fn open_search_panel(
        &mut self,
        id: DocumentId,
        replace_mode: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((query, replacement, editor)) = self.document(id).map(|document| {
            (
                document.search.query.clone(),
                document.search.replacement.clone(),
                document.editor.clone(),
            )
        }) else {
            return;
        };
        self.search_inputs_syncing = true;
        self.search_query
            .update(cx, |input, cx| input.set_value(query, window, cx));
        self.search_replacement
            .update(cx, |input, cx| input.set_value(replacement, window, cx));
        self.search_inputs_syncing = false;
        if let Some(document) = self.document_mut(id) {
            document.search.open = true;
            document.search.replace_mode = replace_mode;
            document.search.error = None;
        }
        editor.update(cx, |state, cx| {
            // 组件自带浮层只用来同步查询，界面由工作区面板负责。
            state.open_search(replace_mode, cx);
            state.close_search(cx);
        });
        self.search_query
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    /// 关闭查找面板，并把焦点还给文档编辑器。
    fn close_search_panel(&mut self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.document(id).map(|document| document.editor.clone()) else {
            return;
        };
        if let Some(document) = self.document_mut(id) {
            document.search.open = false;
        }
        editor.update(cx, |state, cx| {
            state.close_search(cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    /// 修改一项搜索选项后立刻按新选项重新匹配。
    fn toggle_search_option(
        &mut self,
        id: DocumentId,
        toggle: impl FnOnce(&mut SearchOptions),
        cx: &mut Context<Self>,
    ) {
        if let Some(document) = self.document_mut(id) {
            toggle(&mut document.search.options);
            document.search.error = None;
        }
        self.refresh_search(id, cx);
    }

    /// 按当前会话刷新匹配。普通模式交给编辑器，正则模式只记录编译或匹配错误。
    fn refresh_search(&mut self, id: DocumentId, cx: &mut Context<Self>) {
        let Some((editor, session)) = self
            .document(id)
            .map(|document| (document.editor.clone(), document.search.clone()))
        else {
            return;
        };
        if session.options.mode == SearchMode::PlainText {
            editor.update(cx, |state, cx| {
                state.set_search_query(session.query, !session.options.case_sensitive, cx)
            });
            if let Some(document) = self.document_mut(id) {
                document.search.error = None;
            }
        } else {
            let text = editor.read(cx).value().to_string();
            let error = editor::find_matches(&text, &session.query, session.options)
                .err()
                .map(|error| error.to_string());
            if let Some(document) = self.document_mut(id) {
                document.search.error = error;
            }
        }
        cx.notify();
    }

    /// 开关代码折叠。不改变文本，也不把文档标成未保存。
    fn set_document_folding(
        &mut self,
        id: DocumentId,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(document) = self.document_mut(id) else {
            return;
        };
        if document.folding_enabled == enabled {
            return;
        }
        document.folding_enabled = enabled;
        document.editor.update(cx, |state, cx| {
            state.set_folding(enabled, window, cx);
        });
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn document_count(&self) -> usize {
        self.documents.len()
    }

    #[cfg(test)]
    pub(crate) fn active_document_path(&self) -> Option<PathBuf> {
        self.document(self.active_document_id)
            .and_then(|document| document.path.clone())
    }

    #[cfg(test)]
    pub(crate) fn mark_document_saved(&mut self, id: DocumentId) {
        if let Some(document) = self.document_mut(id) {
            document.saved_revision = document.revision;
        }
    }

    #[cfg(test)]
    pub(crate) fn set_document_language_for_test(
        &mut self,
        id: DocumentId,
        language: &str,
        cx: &mut Context<Self>,
    ) {
        self.set_document_language(id, language, cx);
    }

    #[cfg(test)]
    pub(crate) fn document_language_for_test(&self, id: DocumentId) -> Option<String> {
        self.document(id)
            .map(|document| document.language.to_string())
    }

    #[cfg(test)]
    pub(crate) fn set_document_folding_for_test(
        &mut self,
        id: DocumentId,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_folding(id, enabled, window, cx);
    }

    #[cfg(test)]
    pub(crate) fn set_document_line_numbers_for_test(
        &mut self,
        id: DocumentId,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::LineNumbers(value), window, cx);
    }

    #[cfg(test)]
    pub(crate) fn set_document_show_whitespaces_for_test(
        &mut self,
        id: DocumentId,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::Whitespaces(value), window, cx);
    }

    #[cfg(test)]
    pub(crate) fn set_document_smart_indent_for_test(
        &mut self,
        id: DocumentId,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::SmartIndent(value), window, cx);
    }

    #[cfg(test)]
    pub(crate) fn set_document_auto_close_for_test(
        &mut self,
        id: DocumentId,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::AutoClose(value), window, cx);
    }

    #[cfg(test)]
    pub(crate) fn set_document_tab_size_for_test(
        &mut self,
        id: DocumentId,
        value: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::TabSize(value), window, cx);
    }

    #[cfg(test)]
    pub(crate) fn document_settings_for_test(&self, id: DocumentId) -> Option<EditorSettings> {
        self.document(id).map(|document| document.settings.clone())
    }

    #[cfg(test)]
    pub(crate) fn document_folding_for_test(&self, id: DocumentId) -> Option<bool> {
        self.document(id).map(|document| document.folding_enabled)
    }

    #[cfg(test)]
    pub(crate) fn format_document_for_test(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.format_document(id, window, cx);
    }

    /// 加载、确认框、保存或退出还没结束时，忽略会改文档集合的新操作。
    fn is_busy(&self) -> bool {
        self.loading || self.prompting || !self.saving.is_empty() || self.quitting
    }

    /// 当前标签的文档 ID。渲染和动作处理都从这里取，避免中途读到旧下标。
    fn active_id(&self) -> DocumentId {
        self.active_document_id
    }

    /// 路径相同，或两边都能规范化到同一路径时，视为同一个文件。
    fn path_matches(left: &Path, right: &Path) -> bool {
        if left == right {
            return true;
        }
        match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
            (Ok(left), Ok(right)) => left == right,
            _ => false,
        }
    }

    /// 查找已经打开该路径的标签。
    fn find_path(&self, path: &Path) -> Option<DocumentId> {
        self.documents.iter().find_map(|document| {
            document
                .path
                .as_deref()
                .filter(|document_path| Self::path_matches(document_path, path))
                .map(|_| document.id)
        })
    }

    /// 查找除 `id` 以外、已经打开该路径的标签。另存为时用来避免两个标签写同一文件。
    fn find_other_path(&self, id: DocumentId, path: &Path) -> Option<DocumentId> {
        self.documents.iter().find_map(|document| {
            (document.id != id)
                .then_some(document)
                .filter(|document| {
                    document
                        .path
                        .as_deref()
                        .is_some_and(|document_path| Self::path_matches(document_path, path))
                })
                .map(|document| document.id)
        })
    }

    /// 把读入的文件放进标签。路径已经打开时只激活原标签，不用磁盘内容覆盖内存修改。
    fn install_loaded(
        &mut self,
        loaded: file_io::LoadedText,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.find_path(&loaded.path) {
            self.active_document_id = id;
            self.focus_document(id, window, cx);
            return;
        }
        let mut document = self.create_document(
            Some(loaded.path),
            loaded.text,
            loaded.encoding,
            loaded.newline,
            window,
            cx,
        );
        document.disk_fingerprint = Some(loaded.fingerprint);
        self.active_document_id = document.id;
        self.documents.push(document);
        self.focus_document(self.active_document_id, window, cx);
    }

    /// 打开文件选择器，并将用户选中的多个文件逐个后台加载到标签中。
    fn open(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        if self.is_busy() {
            return;
        }
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("打开文本文件".into()),
        });
        self.loading_request_id += 1;
        let request_id = self.loading_request_id;
        self.loading = true;
        cx.spawn_in(window, async move |this, cx| {
            let paths = match picker.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) | Ok(Err(_)) | Err(_) => Vec::new(),
            };
            if paths.is_empty() {
                let _ = this.update_in(cx, |this, _, cx| {
                    if this.loading_request_id == request_id {
                        this.loading = false;
                        this.pending_loads = 0;
                        cx.notify();
                    }
                });
                return;
            }
            let _ = this.update_in(cx, |this, _, cx| {
                if this.loading_request_id == request_id {
                    this.pending_loads = paths.len();
                    cx.notify();
                }
            });
            iter_and_load_files(paths, request_id, this, cx);
        }).detach();
        cx.notify();
    }

    /// 把单个文件的读取结果装进工作区，失败时只更新状态栏。
    fn install_one_result(
        &mut self,
        path: PathBuf,
        result: std::io::Result<file_io::LoadedText>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(loaded) => {
                self.install_loaded(loaded, window, cx);
                self.message = format!("已打开：{}", path.display()).into();
            }
            Err(error) => {
                self.message = format!("打开失败：{}：{error}", path.display()).into();
            }
        }
    }

    /// 保存指定文档。没有路径或选择另存为时先弹出保存对话框。
    ///
    /// 另存为不拿旧指纹做外部修改检查。保存完成时如果期间又有编辑，文档保持 dirty，
    /// 也不会接着执行关闭或退出。
    fn save(
        &mut self,
        id: DocumentId,
        save_as: bool,
        after: Option<AfterSave>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.prompting || self.saving.contains(&id) {
            return;
        }
        let Some(document) = self.document(id) else {
            return;
        };
        // 记下发起保存时的修订。异步返回后若修订已前进，就不能当成已保存。
        let revision = document.revision;
        let text = document.editor.read(cx).value().to_string();
        let encoding = document.encoding;
        let newline = document.newline;
        let path = if save_as { None } else { document.path.clone() };
        let title = document.title.clone();
        let picker = if path.is_none() {
            let dir = document
                .path
                .as_ref()
                .and_then(|path| path.parent())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            Some(cx.prompt_for_new_path(&dir, Some(&title)))
        } else {
            None
        };
        self.saving.insert(id);
        cx.spawn_in(window, async move |this, cx| {
            let chosen = match picker {
                Some(picker) => match picker.await {
                    Ok(Ok(path)) => Ok(path),
                    _ => Err(std::io::Error::other("无法打开保存对话框")),
                },
                None => Ok(path),
            };
            let result = match chosen {
                Ok(Some(path)) => Some(
                    if this
                        .update_in(cx, |this, _, _| this.find_other_path(id, &path))
                        .ok()
                        .flatten()
                        .is_some()
                    {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::AlreadyExists,
                            "目标路径已经在其他标签中打开",
                        ))
                    } else {
                        // 另存为是写到新路径，不拿原文件指纹阻止写入。
                        let expected = this
                            .update_in(cx, |this, _, _| {
                                this.document(id).and_then(|document| {
                                    (!save_as)
                                        .then(|| document.disk_fingerprint.clone())
                                        .flatten()
                                })
                            })
                            .ok()
                            .flatten();
                        cx.background_spawn(async move {
                            file_io::save_if_unchanged(
                                &path,
                                &file_io::bytes_for_save(&text, encoding, newline),
                                expected.as_ref(),
                            )
                        })
                            .await
                    },
                ),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.saving.remove(&id);
                match result {
                    Some(Ok(path)) => {
                        let can_continue = if let Some(document) = this.document_mut(id) {
                            document.title = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                                .into();
                            document.path = Some(path);
                            document.saved_revision = revision;
                            document.disk_fingerprint = file_io::fingerprint(
                                document.path.as_deref().expect("saved path must exist"),
                            )
                                .ok();
                            // 保存期间又编辑过时修订已经前进，不能接着关闭或退出。
                            !document.is_dirty()
                        } else {
                            return;
                        };
                        this.message = "已保存".into();
                        if can_continue {
                            this.after_save(id, after, window, cx);
                        }
                    }
                    Some(Err(error)) => {
                        // 保存失败要停掉整次退出，避免后面的标签在用户没看到错误时被关掉。
                        this.quitting = false;
                        this.quit_discarded.clear();
                        this.message = format!("保存失败：{error}").into();
                    }
                    None => {
                        this.quitting = false;
                        this.quit_discarded.clear();
                        this.message = "已取消保存".into();
                    }
                }
                cx.notify();
            });
        })
            .detach();
        cx.notify();
    }

    /// 保存成功且文档已干净时，继续关闭该标签或接着询问下一个未保存文档。
    fn after_save(
        &mut self,
        id: DocumentId,
        after: Option<AfterSave>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match after {
            Some(AfterSave::Close(close_id)) if close_id == id => {
                self.remove_document(close_id, window, cx);
            }
            Some(AfterSave::Quit) => self.continue_quit(window, cx),
            _ => {}
        }
    }

    /// 关闭标签。有未保存修改时先询问保存、放弃或取消。
    fn request_close_document(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_busy() {
            return;
        }
        if !self.document(id).is_some_and(Document::is_dirty) {
            self.remove_document(id, window, cx);
            return;
        }
        self.prompting = true;
        let prompt = window.prompt(
            PromptLevel::Warning,
            "保存文档的修改？",
            Some("关闭标签会丢弃未保存的修改。"),
            &["保存", "放弃修改", "取消"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let result = prompt.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.prompting = false;
                match result {
                    Ok(0) => this.save(id, false, Some(AfterSave::Close(id)), window, cx),
                    Ok(1) => this.remove_document(id, window, cx),
                    _ => {}
                }
                cx.notify();
            });
        })
            .detach();
        cx.notify();
    }

    /// 从标签栏移除文档。最后一个标签被关掉时换成一份空白未命名文档。
    fn remove_document(&mut self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.documents.iter().position(|document| document.id == id) else {
            return;
        };
        self._subscriptions
            .retain(|(document_id, _)| *document_id != id);
        // 窗口始终至少保留一个标签，避免编辑器实体和焦点一起消失。
        if self.documents.len() == 1 {
            let replacement =
                self.create_document(None, String::new(), Encoding::Utf8, Newline::Lf, window, cx);
            self.documents[index] = replacement;
            self.active_document_id = self.documents[index].id;
        } else {
            self.documents.remove(index);
            if self.active_document_id == id {
                let next_index = index.min(self.documents.len() - 1);
                self.active_document_id = self.documents[next_index].id;
            }
        }
        self.focus_document(self.active_document_id, window, cx);
        cx.notify();
    }

    /// 开始退出。加载、保存或确认框还没结束时直接忽略这次请求。
    fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || !self.saving.is_empty() || self.prompting || self.quitting {
            return;
        }
        self.quit_discarded.clear();
        self.quitting = true;
        self.continue_quit(window, cx);
    }

    /// 找到下一个仍 dirty 且本次尚未放弃的文档并询问；没有了才真正关窗。
    fn continue_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(document) = self
            .documents
            .iter()
            .find(|document| document.is_dirty() && !self.quit_discarded.contains(&document.id))
        else {
            self.quitting = false;
            self.quit_discarded.clear();
            window.remove_window();
            return;
        };
        let id = document.id;
        self.prompting = true;
        let prompt = window.prompt(
            PromptLevel::Warning,
            "保存文档的修改？",
            Some("退出前需要处理所有未保存的文档。"),
            &["保存", "放弃修改", "取消"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let result = prompt.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.prompting = false;
                match result {
                    Ok(0) => this.save(id, false, Some(AfterSave::Quit), window, cx),
                    Ok(1) => {
                        // 放弃只跳过这次询问，不改磁盘，也不把内存修订标成已保存。
                        this.quit_discarded.insert(id);
                        this.continue_quit(window, cx);
                    }
                    _ => {
                        this.quitting = false;
                        this.quit_discarded.clear();
                    }
                }
                cx.notify();
            });
        })
            .detach();
        cx.notify();
    }

    /// 格式化当前文档。JSON 走严格解析和容错修复，其他文本只清理行尾空白。
    fn format_document(&mut self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
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

    /// 处理窗口关闭请求，完成确认后才真正移除窗口。
    pub(super) fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_quit(window, cx);
    }

    /// 把拖入的文件按打开流程逐个加载。忙碌时忽略，避免和正在进行的加载交错。
    fn drop_files(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() {
            return;
        }
        let paths = paths.paths().to_vec();
        if paths.is_empty() {
            return;
        }
        self.loading_request_id += 1;
        let request_id = self.loading_request_id;
        self.loading = true;
        self.pending_loads = paths.len();
        cx.spawn_in(window, async move |this, cx| {
            // 遍历 paths 中的每个路径，并调用 file_io::load 函数加载文件。
            iter_and_load_files(paths, request_id, this, cx);
        }).detach();
        cx.notify();
    }

    /// 构造 Markdown 预览面板。
    ///
    /// 预览内容跟随编辑器正文实时刷新；正文为空时给出一段占位提示，
    /// 避免面板看起来像没有加载出来。
    fn build_preview_panel(&self, document: &Document, cx: &App) -> AnyElement {
        let text = document.editor.read(cx).value();
        let body = if text.trim().is_empty() {
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("没有可预览的内容")
                .into_any_element()
        } else {
            // 元素 ID 带上文档 ID，切换标签时预览状态各自独立保留。
            TextView::markdown(format!("markdown-preview-{}", document.id.0), text.clone())
                .scrollable(true)
                .selectable(true)
                .into_any_element()
        };

        div()
            .id(format!("markdown-preview-panel-{}", document.id.0))
            .aria_label("Markdown 预览")
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .px_6()
            .py_4()
            .child(body)
            // 让 UI 测试能观测到这个面板；普通构建下这一步不做任何事。
            .test_support()
            .into_any_element()
    }
}

fn iter_and_load_files(paths: Vec<PathBuf>, request_id: u64, this: WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    for path in paths {
        let weak = this.clone();
        let task = cx.background_spawn(async move {
            let result = file_io::load(&path);
            (path, result)
        });
        cx.spawn(async move |cx| {
            let (path, result) = task.await;
            let _ = weak.update_in(cx, |this, window, cx| {
                // 期间又发起了新的打开，这份结果已经过期。
                if this.loading_request_id != request_id {
                    return;
                }
                this.install_one_result(path, result, window, cx);
                this.pending_loads = this.pending_loads.saturating_sub(1);
                this.loading = this.pending_loads != 0;
                cx.notify();
            });
        }).detach();
    }
}

impl Render for Workspace {
    /// 绘制标题、标签、查找面板、正文和状态栏，并在这里挂上全部动作。
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_id = self.active_id();
        let active_index = self
            .documents
            .iter()
            .position(|document| document.id == active_id)
            .unwrap_or(0);
        let active = self.document(active_id);
        let position = active.map(|document| document.editor.read(cx).cursor_position());
        let title = active
            .map(|document| document.title.clone())
            .unwrap_or_else(|| "未命名".into());
        let active_dirty = active.is_some_and(Document::is_dirty);
        let tabs = self.documents.iter().map(|document| {
            let id = document.id;
            let label = format!(
                "{}{}",
                document.title,
                if document.is_dirty() { " *" } else { "" }
            );
            Tab::new()
                .label(label)
                .selected(id == active_id)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.activate_document_with_focus(id, window, cx);
                    cx.notify();
                }))
                .suffix(
                    Button::new(format!("close-tab-{}", id.0))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .accessibility_label("关闭标签")
                        .tooltip("关闭标签")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.request_close_document(id, window, cx)
                        })),
                )
        });
        let can_edit = !self.is_busy();
        let language_value = active
            .map(|document| document.language.clone())
            .unwrap_or_else(|| "plaintext".into());
        div()
            .id("rueditor")
            .track_focus(&self.focus)
            .v_flex()
            .size_full()
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.drop_files(paths, window, cx)
            }))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &New, window, cx| {
                if !this.is_busy() {
                    this.new_tab(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Open, window, cx| this.open(cx, window)))
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                let id = this.active_id();
                this.save(id, false, None, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveAs, window, cx| {
                let id = this.active_id();
                this.save(id, true, None, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Format, window, cx| {
                let id = this.active_id();
                this.format_document(id, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Close, window, cx| {
                let id = this.active_id();
                this.request_close_document(id, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Quit, window, cx| {
                this.request_quit(window, cx);
            }))
            .on_action(cx.listener(|_, _: &ShowAbout, window, cx| {
                window.open_alert_dialog(cx, |alert, _, _| {
                    alert
                        .title("关于 Rueditor")
                        .description("轻量级跨平台文本编辑器")
                });
            }))
            .on_action(cx.listener(|this, _: &UppercaseSelection, window, cx| {
                this.transform_selection(
                    this.active_id(),
                    editor::TextTransform::Uppercase,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &LowercaseSelection, window, cx| {
                this.transform_selection(
                    this.active_id(),
                    editor::TextTransform::Lowercase,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &DeleteLine, window, cx| {
                this.transform_line(this.active_id(), editor::LineTransform::Delete, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveLineUp, window, cx| {
                this.transform_line(this.active_id(), editor::LineTransform::MoveUp, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveLineDown, window, cx| {
                this.transform_line(
                    this.active_id(),
                    editor::LineTransform::MoveDown,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &IndentSelection, window, cx| {
                this.transform_line(this.active_id(), editor::LineTransform::Indent, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OutdentSelection, window, cx| {
                this.transform_line(this.active_id(), editor::LineTransform::Outdent, window, cx);
            }))
            .on_action(cx.listener(|this, _: &DuplicateLine, window, cx| {
                this.duplicate_current_line(this.active_id(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &TrimTrailingWhitespace, window, cx| {
                this.trim_trailing_whitespace(this.active_id(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &GotoLine, window, cx| {
                let id = this.active_id();
                let input = cx.new(|cx| InputState::new(window, cx));
                let weak = cx.entity().downgrade();
                window.open_dialog(cx, move |dialog, _, _| {
                    let input_for_confirm = input.clone();
                    let input_for_content = input.clone();
                    let weak = weak.clone();
                    dialog
                        .title("跳转到行")
                        .content(move |content, _, _| content.child(Input::new(&input_for_content)))
                        .button_props(
                            component::dialog::DialogButtonProps::default()
                                .ok_text("跳转")
                                .show_cancel(true)
                                .on_ok(move |_, window, app| {
                                    let value = input_for_confirm.read(app).value().to_string();
                                    let _ = weak.update(app, |this, cx| {
                                        this.goto_line(id, &value, window, cx);
                                    });
                                    true
                                }),
                        )
                });
            }))
            .on_action(cx.listener(|this, _: &SetPlaintext, _, cx| {
                this.set_document_language(this.active_id(), "plaintext", cx);
            }))
            .on_action(cx.listener(|this, _: &SetRust, _, cx| {
                this.set_document_language(this.active_id(), "rust", cx);
            }))
            .on_action(cx.listener(|this, _: &SetPython, _, cx| {
                this.set_document_language(this.active_id(), "python", cx);
            }))
            .on_action(cx.listener(|this, _: &SetJavaScript, _, cx| {
                this.set_document_language(this.active_id(), "javascript", cx);
            }))
            .on_action(cx.listener(|this, _: &SetTypeScript, _, cx| {
                this.set_document_language(this.active_id(), "typescript", cx);
            }))
            .on_action(cx.listener(|this, _: &SetTsx, _, cx| {
                this.set_document_language(this.active_id(), "tsx", cx);
            }))
            .on_action(cx.listener(|this, _: &SetJson, _, cx| {
                this.set_document_language(this.active_id(), "json", cx);
            }))
            .on_action(cx.listener(|this, _: &SetHtml, _, cx| {
                this.set_document_language(this.active_id(), "html", cx);
            }))
            .on_action(cx.listener(|this, _: &SetCss, _, cx| {
                this.set_document_language(this.active_id(), "css", cx);
            }))
            .on_action(cx.listener(|this, _: &SetMarkdown, _, cx| {
                this.set_document_language(this.active_id(), "markdown", cx);
            }))
            .on_action(cx.listener(|this, _: &SetToml, _, cx| {
                this.set_document_language(this.active_id(), "toml", cx);
            }))
            .on_action(cx.listener(|this, _: &SetYaml, _, cx| {
                this.set_document_language(this.active_id(), "yaml", cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleFolding, window, cx| {
                let id = this.active_id();
                let enabled = this
                    .document(id)
                    .is_some_and(|document| !document.folding_enabled);
                this.set_document_folding(id, enabled, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleLineNumbers, window, cx| {
                this.toggle_document_setting(
                    this.active_id(),
                    |s| s.show_line_numbers,
                    SettingChange::LineNumbers,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &ToggleSoftWrap, window, cx| {
                this.toggle_document_setting(
                    this.active_id(),
                    |s| s.soft_wrap,
                    SettingChange::SoftWrap,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &ToggleShowWhitespaces, window, cx| {
                this.toggle_document_setting(
                    this.active_id(),
                    |s| s.show_whitespaces,
                    SettingChange::Whitespaces,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &ToggleSmartIndent, window, cx| {
                this.toggle_document_setting(
                    this.active_id(),
                    |s| s.smart_indent,
                    SettingChange::SmartIndent,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &ToggleAutoClose, window, cx| {
                this.toggle_document_setting(
                    this.active_id(),
                    |s| s.auto_close,
                    SettingChange::AutoClose,
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &TogglePreview, window, cx| {
                this.toggle_preview(this.active_id(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &SetTabSize2, window, cx| {
                this.set_document_setting(this.active_id(), SettingChange::TabSize(2), window, cx);
            }))
            .on_action(cx.listener(|this, _: &SetTabSize4, window, cx| {
                this.set_document_setting(this.active_id(), SettingChange::TabSize(4), window, cx);
            }))
            .on_action(cx.listener(|this, _: &SetTabSize8, window, cx| {
                this.set_document_setting(this.active_id(), SettingChange::TabSize(8), window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindNext, _, cx| {
                this.find_next(this.active_id(), cx);
            }))
            .on_action(cx.listener(|this, _: &FindPrevious, _, cx| {
                this.find_previous(this.active_id(), cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSearchPanel, window, cx| {
                this.open_search_panel(this.active_id(), false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Search, window, cx| {
                this.open_search_panel(this.active_id(), false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Replace, window, cx| {
                this.open_search_panel(this.active_id(), true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleRegex, _, cx| {
                let id = this.active_id();
                this.toggle_search_option(
                    id,
                    |options| {
                        options.mode = if options.mode == SearchMode::Regex {
                            SearchMode::PlainText
                        } else {
                            SearchMode::Regex
                        };
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &ToggleCaseSensitive, _, cx| {
                let id = this.active_id();
                this.toggle_search_option(
                    id,
                    |options| options.case_sensitive = !options.case_sensitive,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &ToggleWholeWord, _, cx| {
                let id = this.active_id();
                this.toggle_search_option(
                    id,
                    |options| options.whole_word = !options.whole_word,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &ReplaceCurrent, window, cx| {
                this.replace_current(this.active_id(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReplaceAllMatches, window, cx| {
                this.replace_all_matches(this.active_id(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseSearchPanel, window, cx| {
                this.close_search_panel(this.active_id(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &Escape, window, cx| {
                if this
                    .document(this.active_id())
                    .is_some_and(|document| document.search.open)
                {
                    this.close_search_panel(this.active_id(), window, cx);
                }
            }))
            .child(
                div()
                    .h_flex()
                    .h_8()
                    .px_3()
                    .flex_shrink_0()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(format!("{}{}", title, if active_dirty { " *" } else { "" }))
                    .child(div().flex_1())
                    .when_some(
                        // 只有支持预览的语言才显示这个按钮，避免给不可用的操作入口。
                        active.filter(|document| {
                            editor::supports_preview(document.language.as_ref())
                        }),
                        |bar, document| {
                            let enabled = document.settings.show_preview;
                            bar.child(
                                Button::new("toggle-preview")
                                    .ghost()
                                    .small()
                                    .icon(IconName::BookOpen)
                                    .selected(enabled)
                                    .accessibility_label("切换 Markdown 预览")
                                    .tooltip("切换 Markdown 预览")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_preview(this.active_id(), window, cx);
                                    })),
                            )
                        },
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} · {} · {}",
                                editor::language_label(language_value.as_ref()),
                                active.map_or("Utf8", |document| match document.encoding {
                                    Encoding::Utf8 => "Utf8",
                                    Encoding::Utf8Bom => "Utf8-BOM",
                                }),
                                active.map_or("Lf", |document| match document.newline {
                                    Newline::Lf => "Lf",
                                    Newline::Crlf => "CrLf",
                                })
                            )),
                    ),
            )
            .child(
                TabBar::new("documents")
                    .underline()
                    .small()
                    .selected_index(active_index)
                    .children(tabs)
                    .suffix(
                        Button::new("new-tab")
                            .ghost()
                            .small()
                            .icon(IconName::Plus)
                            .accessibility_label("新建标签")
                            .tooltip("新建标签")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_tab(window, cx);
                            })),
                    ),
            )
            .child(if active.is_some_and(|document| document.search.open) {
                let session = active
                    .map(|document| document.search.clone())
                    .unwrap_or_default();
                let matches = active
                    .and_then(|document| {
                        editor::find_matches(
                            document.editor.read(cx).value().as_ref(),
                            &document.search.query,
                            document.search.options,
                        )
                            .ok()
                    })
                    .map_or(0, |ranges| ranges.len());
                let id = active_id;
                div()
                    .v_flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .items_center()
                            .child(Input::new(&self.search_query).small())
                            .child(format!(
                                "{} · {}",
                                if session.options.mode == SearchMode::Regex {
                                    "正则"
                                } else {
                                    "文本"
                                },
                                matches
                            ))
                            .child(
                                Button::new("search-next")
                                    .ghost()
                                    .small()
                                    .icon(IconName::ChevronDown)
                                    .tooltip("下一个")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.find_next(this.active_id(), cx)
                                    })),
                            )
                            .child(
                                Button::new("search-previous")
                                    .ghost()
                                    .small()
                                    .icon(IconName::ChevronUp)
                                    .tooltip("上一个")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.find_previous(this.active_id(), cx)
                                    })),
                            )
                            .child(
                                Button::new("search-regex")
                                    .ghost()
                                    .small()
                                    .label("正则")
                                    .selected(session.options.mode == SearchMode::Regex)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let id = this.active_id();
                                        this.toggle_search_option(
                                            id,
                                            |options| {
                                                options.mode = if options.mode == SearchMode::Regex
                                                {
                                                    SearchMode::PlainText
                                                } else {
                                                    SearchMode::Regex
                                                }
                                            },
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("search-case")
                                    .ghost()
                                    .small()
                                    .label("Aa")
                                    .selected(session.options.case_sensitive)
                                    .tooltip("区分大小写")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let id = this.active_id();
                                        this.toggle_search_option(
                                            id,
                                            |options| {
                                                options.case_sensitive = !options.case_sensitive
                                            },
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("search-word")
                                    .ghost()
                                    .small()
                                    .label("词")
                                    .selected(session.options.whole_word)
                                    .tooltip("全词匹配")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let id = this.active_id();
                                        this.toggle_search_option(
                                            id,
                                            |options| options.whole_word = !options.whole_word,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("search-close")
                                    .ghost()
                                    .small()
                                    .icon(IconName::Close)
                                    .tooltip("关闭查找")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.close_search_panel(id, window, cx)
                                    })),
                            ),
                    )
                    .when(session.replace_mode, |panel| {
                        panel.child(
                            div()
                                .h_flex()
                                .gap_2()
                                .items_center()
                                .child(Input::new(&self.search_replacement).small())
                                .child(
                                    Button::new("replace-current")
                                        .outline()
                                        .small()
                                        .label("替换")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.replace_current(this.active_id(), window, cx)
                                        })),
                                )
                                .child(
                                    Button::new("replace-all")
                                        .outline()
                                        .small()
                                        .label("全部替换")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.replace_all_matches(this.active_id(), window, cx)
                                        })),
                                ),
                        )
                    })
                    .when_some(session.error, |panel, error| {
                        panel.child(div().text_xs().text_color(cx.theme().danger).child(error))
                    })
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(if let Some(document) = active {
                        let editor_panel = Editor::new(&document.editor)
                            .aria_label("文档正文")
                            .readonly(!can_edit)
                            .h(relative(1.))
                            .into_any_element();
                        // 预览只在受支持的文档上出现，且由文档自己的设置决定。
                        let show_preview = document.settings.show_preview
                            && editor::supports_preview(document.language.as_ref());
                        if show_preview {
                            let preview = self.build_preview_panel(document, cx);
                            h_resizable("markdown-preview-split")
                                .child(resizable_panel().child(editor_panel))
                                .child(resizable_panel().child(preview))
                                .into_any_element()
                        } else {
                            editor_panel
                        }
                    } else {
                        div().size_full().into_any_element()
                    }),
            )
            .child(
                StatusBar::new()
                    .left(if let Some(position) = position {
                        format!("行 {}，列 {}", position.line + 1, position.character + 1)
                    } else {
                        "未命名".to_string()
                    })
                    .child(if self.loading {
                        "处理中…".into()
                    } else {
                        self.message.clone()
                    }),
            )
    }
}
