// 现有工作区集成测试使用的状态和动作入口。
use super::super::{
    document::{Document, DocumentId, EditorSettings, SearchSession},
    editor,
};
use super::{Workspace, settings::SettingChange};
use gpui_kit::*;
use std::path::PathBuf;

impl Workspace {
    pub(crate) fn activate_document_for_test(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.activate_document_with_focus(id, window, cx)
    }

    /// 当前文档的稳定 ID。
    pub(in crate::text_editor) fn active_document_id(&self) -> DocumentId {
        self.active_document_id
    }

    /// 测试和工作区逻辑使用的文档文本读取入口。
    pub(in crate::text_editor) fn document_text(&self, id: DocumentId, cx: &App) -> Option<String> {
        self.document(id)
            .map(|document| document.editor.read(cx).value().to_string())
    }

    /// 查询文档是否有未保存修改。
    pub(in crate::text_editor) fn document_is_dirty(&self, id: DocumentId) -> bool {
        self.document(id).is_some_and(Document::is_dirty)
    }

    pub(in crate::text_editor) fn document_revision_for_test(&self, id: DocumentId) -> Option<u64> {
        self.document(id).map(|document| document.revision)
    }

    pub(in crate::text_editor) fn set_search_session_for_test(
        &mut self,
        id: DocumentId,
        session: SearchSession,
    ) {
        if let Some(document) = self.document_mut(id) {
            document.search = session;
        }
    }

    pub(in crate::text_editor) fn search_session_for_test(
        &self,
        id: DocumentId,
    ) -> Option<SearchSession> {
        self.document(id).map(|document| document.search.clone())
    }

    /// 测试用的文档编辑入口，模拟真实编辑后保持各文档修订独立。
    pub(in crate::text_editor) fn set_document_value(
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

    pub(crate) fn goto_line_for_test(
        &mut self,
        id: DocumentId,
        input: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.goto_line(id, input, window, cx)
    }

    pub(crate) fn document_cursor_for_test(&self, id: DocumentId, cx: &App) -> Option<(u32, u32)> {
        self.document(id).map(|document| {
            let position = document.editor.read(cx).cursor_position();
            (position.line, position.character)
        })
    }

    pub(crate) fn message_for_test(&self) -> &str {
        &self.message
    }

    /// 测试用的预览开关查询入口。
    pub(in crate::text_editor) fn document_preview_enabled_for_test(
        &self,
        id: DocumentId,
    ) -> Option<bool> {
        self.document(id)
            .map(|document| document.settings.show_preview)
    }

    /// 测试用的预览开关入口。
    pub(crate) fn toggle_preview_for_test(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_preview(id, window, cx);
    }

    pub(crate) fn document_count(&self) -> usize {
        self.documents.len()
    }

    pub(crate) fn active_document_path(&self) -> Option<PathBuf> {
        self.document(self.active_document_id)
            .and_then(|document| document.path.clone())
    }

    pub(crate) fn mark_document_saved(&mut self, id: DocumentId) {
        if let Some(document) = self.document_mut(id) {
            document.saved_revision = document.revision;
        }
    }

    pub(crate) fn set_document_language_for_test(
        &mut self,
        id: DocumentId,
        language: &str,
        cx: &mut Context<Self>,
    ) {
        self.set_document_language(id, language, cx);
    }

    pub(crate) fn document_language_for_test(&self, id: DocumentId) -> Option<String> {
        self.document(id)
            .map(|document| document.language.to_string())
    }

    pub(crate) fn set_document_folding_for_test(
        &mut self,
        id: DocumentId,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_folding(id, enabled, window, cx);
    }

    pub(crate) fn set_document_line_numbers_for_test(
        &mut self,
        id: DocumentId,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::LineNumbers(value), window, cx);
    }

    pub(crate) fn set_document_show_whitespaces_for_test(
        &mut self,
        id: DocumentId,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::Whitespaces(value), window, cx);
    }

    pub(crate) fn set_document_smart_indent_for_test(
        &mut self,
        id: DocumentId,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::SmartIndent(value), window, cx);
    }

    pub(crate) fn set_document_auto_close_for_test(
        &mut self,
        id: DocumentId,
        value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::AutoClose(value), window, cx);
    }

    pub(crate) fn set_document_tab_size_for_test(
        &mut self,
        id: DocumentId,
        value: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_document_setting(id, SettingChange::TabSize(value), window, cx);
    }

    pub(crate) fn document_settings_for_test(&self, id: DocumentId) -> Option<EditorSettings> {
        self.document(id).map(|document| document.settings.clone())
    }

    pub(crate) fn document_folding_for_test(&self, id: DocumentId) -> Option<bool> {
        self.document(id).map(|document| document.folding_enabled)
    }

    pub(crate) fn document_fold_candidate_for_test(
        &self,
        id: DocumentId,
        line: usize,
        cx: &App,
    ) -> Option<bool> {
        self.document(id).map(|document| {
            document
                .editor
                .read(cx)
                .display_map()
                .is_fold_candidate(line)
        })
    }

    pub(crate) fn format_document_for_test(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.format_document(id, window, cx);
    }
}
