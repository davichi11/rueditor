// 工作区根节点的菜单与快捷键动作。
use super::super::{document::SearchMode, editor};
use super::{Workspace, settings::SettingChange};
use gpui_kit::{
    component::{
        WindowExt as _,
        input::{Escape, Input, InputState, Replace, Search},
    },
    *,
};

use editor::{
    Close, CloseSearchPanel, DeleteLine, DuplicateLine, FindNext, FindPrevious, Format, GotoLine,
    IndentSelection, LowercaseSelection, MoveLineDown, MoveLineUp, New, Open, OpenSearchPanel,
    OutdentSelection, Quit, ReplaceAllMatches, ReplaceCurrent, Save, SaveAs, SetBash, SetC,
    SetCMake, SetCSharp, SetCpp, SetCss, SetDiff, SetDotenv, SetElixir, SetGo, SetGraphql, SetHtml,
    SetIni, SetJava, SetJavaScript, SetJson, SetJsx, SetKotlin, SetLua, SetMake, SetMarkdown,
    SetPhp, SetPlaintext, SetProperties, SetProto, SetPython, SetRuby, SetRust, SetScala, SetSql,
    SetSwift, SetTabSize2, SetTabSize4, SetTabSize8, SetToml, SetTsx, SetTypeScript, SetXml,
    SetYaml, SetZig, ShowAbout, ToggleAutoClose, ToggleCaseSensitive, ToggleFolding,
    ToggleLineNumbers, TogglePreview, ToggleRegex, ToggleShowWhitespaces, ToggleSmartIndent,
    ToggleSoftWrap, ToggleWholeWord, TrimTrailingWhitespace, UppercaseSelection,
};

impl Workspace {
    pub(super) fn bind_actions(root: Stateful<Div>, cx: &Context<Self>) -> Stateful<Div> {
        root.on_action(cx.listener(|this, _: &New, window, cx| {
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
        .on_action(cx.listener(|this, _: &SetC, _, cx| {
            this.set_document_language(this.active_id(), "c", cx);
        }))
        .on_action(cx.listener(|this, _: &SetCpp, _, cx| {
            this.set_document_language(this.active_id(), "cpp", cx);
        }))
        .on_action(cx.listener(|this, _: &SetJava, _, cx| {
            this.set_document_language(this.active_id(), "java", cx);
        }))
        .on_action(cx.listener(|this, _: &SetGo, _, cx| {
            this.set_document_language(this.active_id(), "go", cx);
        }))
        .on_action(cx.listener(|this, _: &SetCSharp, _, cx| {
            this.set_document_language(this.active_id(), "csharp", cx);
        }))
        .on_action(cx.listener(|this, _: &SetKotlin, _, cx| {
            this.set_document_language(this.active_id(), "kotlin", cx);
        }))
        .on_action(cx.listener(|this, _: &SetSwift, _, cx| {
            this.set_document_language(this.active_id(), "swift", cx);
        }))
        .on_action(cx.listener(|this, _: &SetPhp, _, cx| {
            this.set_document_language(this.active_id(), "php", cx);
        }))
        .on_action(cx.listener(|this, _: &SetRuby, _, cx| {
            this.set_document_language(this.active_id(), "ruby", cx);
        }))
        .on_action(cx.listener(|this, _: &SetLua, _, cx| {
            this.set_document_language(this.active_id(), "lua", cx);
        }))
        .on_action(cx.listener(|this, _: &SetScala, _, cx| {
            this.set_document_language(this.active_id(), "scala", cx);
        }))
        .on_action(cx.listener(|this, _: &SetElixir, _, cx| {
            this.set_document_language(this.active_id(), "elixir", cx);
        }))
        .on_action(cx.listener(|this, _: &SetZig, _, cx| {
            this.set_document_language(this.active_id(), "zig", cx);
        }))
        .on_action(cx.listener(|this, _: &SetSql, _, cx| {
            this.set_document_language(this.active_id(), "sql", cx);
        }))
        .on_action(cx.listener(|this, _: &SetBash, _, cx| {
            this.set_document_language(this.active_id(), "bash", cx);
        }))
        .on_action(cx.listener(|this, _: &SetMake, _, cx| {
            this.set_document_language(this.active_id(), "make", cx);
        }))
        .on_action(cx.listener(|this, _: &SetCMake, _, cx| {
            this.set_document_language(this.active_id(), "cmake", cx);
        }))
        .on_action(cx.listener(|this, _: &SetProto, _, cx| {
            this.set_document_language(this.active_id(), "proto", cx);
        }))
        .on_action(cx.listener(|this, _: &SetGraphql, _, cx| {
            this.set_document_language(this.active_id(), "graphql", cx);
        }))
        .on_action(cx.listener(|this, _: &SetDiff, _, cx| {
            this.set_document_language(this.active_id(), "diff", cx);
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
        .on_action(cx.listener(|this, _: &SetXml, _, cx| {
            this.set_document_language(this.active_id(), "xml", cx);
        }))
        .on_action(cx.listener(|this, _: &SetProperties, _, cx| {
            this.set_document_language(this.active_id(), "properties", cx);
        }))
        .on_action(cx.listener(|this, _: &SetIni, _, cx| {
            this.set_document_language(this.active_id(), "ini", cx);
        }))
        .on_action(cx.listener(|this, _: &SetDotenv, _, cx| {
            this.set_document_language(this.active_id(), "dotenv", cx);
        }))
        .on_action(cx.listener(|this, _: &SetJsx, _, cx| {
            this.set_document_language(this.active_id(), "jsx", cx);
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
            this.toggle_search_option(id, |options| options.whole_word = !options.whole_word, cx);
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
    }
}
