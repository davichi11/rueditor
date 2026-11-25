// 文档级编辑器配置及预览开关。
use super::super::{
    document::{DocumentId, EditorSettings},
    editor,
};
use super::Workspace;
use gpui_kit::component::input::EditorState;
use gpui_kit::*;

/// 一次只改文档设置里的一项，避免切换时把其他项重置成默认值。
#[derive(Clone, Copy)]
pub(super) enum SettingChange {
    LineNumbers(bool),
    SoftWrap(bool),
    Whitespaces(bool),
    SmartIndent(bool),
    AutoClose(bool),
    TabSize(usize),
    Preview(bool),
}

/// 把文档设置整份写进编辑器实体。切换标签时用来恢复该文档自己的显示。
pub(super) fn apply_editor_settings(
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

impl Workspace {
    /// 更换高亮语言。文本和修订号保持不变。
    pub(super) fn set_document_language(
        &mut self,
        id: DocumentId,
        language: &str,
        cx: &mut Context<Self>,
    ) {
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
    pub(super) fn set_document_setting(
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
    pub(super) fn toggle_document_setting(
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
    pub(super) fn toggle_preview(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    /// 开关代码折叠。不改变文本，也不把文档标成未保存。
    pub(super) fn set_document_folding(
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
            if enabled {
                // Reapply the adapter's cached candidates after edits made
                // while folding was disabled.
                state.refresh(cx);
            }
        });
        cx.notify();
    }
}
