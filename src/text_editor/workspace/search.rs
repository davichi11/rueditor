// 查找替换会话及面板。
use super::super::{
    document::{Document, DocumentId, SearchMode, SearchOptions},
    editor,
};
use super::Workspace;
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Workspace {
    /// 用指定文档的搜索会话覆盖共用输入框，期间不把这次写入回写到会话。
    pub(super) fn sync_search_inputs(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    /// 从当前位置向后查找下一个匹配，到末尾后绕回开头。
    pub(super) fn find_next(&mut self, id: DocumentId, cx: &mut Context<Self>) {
        self.navigate_search(id, true, cx);
    }

    /// 从当前位置向前查找上一个匹配，到开头后绕回末尾。
    pub(super) fn find_previous(&mut self, id: DocumentId, cx: &mut Context<Self>) {
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
                // Update the matcher directly without opening a component overlay.
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
    pub(super) fn replace_current(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(document) = self.document(id) else {
            return;
        };
        let session = document.search.clone();
        let editor_state = document.editor.clone();
        let text = editor_state.read(cx).value().to_string();
        if session.options.mode == SearchMode::PlainText {
            editor_state.update(cx, |state, cx| {
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
    pub(super) fn replace_all_matches(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(document) = self.document(id) else {
            return;
        };
        let session = document.search.clone();
        let editor_state = document.editor.clone();
        if session.options.mode == SearchMode::PlainText {
            editor_state.update(cx, |state, cx| {
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
    pub(super) fn open_search_panel(
        &mut self,
        id: DocumentId,
        replace_mode: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        if let Some(document) = self.document_mut(id) {
            document.search.open = true;
            document.search.replace_mode = replace_mode;
            document.search.error = None;
        }
        self.refresh_search(id, cx);
        self.search_query
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    /// 关闭查找面板，并把焦点还给文档编辑器。
    pub(super) fn close_search_panel(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
    pub(super) fn toggle_search_option(
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
    pub(super) fn refresh_search(&mut self, id: DocumentId, cx: &mut Context<Self>) {
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

    pub(super) fn render_search_panel(
        &self,
        active: Option<&Document>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(document) = active.filter(|document| document.search.open) else {
            return div().into_any_element();
        };
        let session = document.search.clone();
        let matches = editor::find_matches(
            document.editor.read(cx).value().as_ref(),
            &session.query,
            session.options,
        )
        .map_or(0, |ranges| ranges.len());
        let id = document.id;
        div()
            .v_flex()
            .gap_2()
            .px_3()
            .py_2()
            .bg(cx.theme().muted)
            .text_xs()
            .flex_shrink_0()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.search_query).small()),
                    )
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} 项匹配", matches)),
                    )
                    .child(
                        Button::new("search-next")
                            .accessibility_label("下一个匹配")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronDown)
                            .tooltip("下一个")
                            .on_click(
                                cx.listener(|this, _, _, cx| this.find_next(this.active_id(), cx)),
                            ),
                    )
                    .child(
                        Button::new("search-previous")
                            .accessibility_label("上一个匹配")
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
                                        options.mode = if options.mode == SearchMode::Regex {
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
                                    |options| options.case_sensitive = !options.case_sensitive,
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
                            .accessibility_label("关闭查找")
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
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Input::new(&self.search_replacement).small()),
                        )
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
    }
}
