// 工作区的标签栏、工具栏、正文与状态栏。
use super::super::{
    chrome,
    document::{Document, Encoding},
    editor,
};
use super::Workspace;
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Editor,
    menu::{DropdownMenu, PopupMenu},
    resizable::{h_resizable, resizable_panel},
    status_bar::StatusBar,
    tab::{Tab, TabBar},
    text::TextView,
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.document(self.active_id());
        let can_edit = !self.is_busy();
        let root = div()
            .id("rueditor")
            .track_focus(&self.focus)
            .v_flex()
            .size_full()
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.drop_files(paths, window, cx)
            }))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground);

        Self::bind_actions(root, cx)
            .child(self.render_tabs(cx))
            .child(self.render_toolbar(active, can_edit, cx))
            .child(self.render_search_panel(active, cx))
            .child(self.render_editor(active, can_edit, cx))
            .child(self.render_status(active, cx))
    }
}

impl Workspace {
    fn render_tabs(&self, cx: &Context<Self>) -> impl IntoElement {
        let active_id = self.active_id();
        let active_index = self
            .documents
            .iter()
            .position(|document| document.id == active_id)
            .unwrap_or(0);
        let tabs = self.documents.iter().map(|document| {
            let id = document.id;
            Tab::new()
                .label(document.title.clone())
                .aria_label(format!(
                    "{}{}",
                    chrome::location(document),
                    if document.is_dirty() {
                        "，未保存"
                    } else {
                        ""
                    }
                ))
                .prefix(
                    div()
                        .id(format!("document-location-{}", id.0))
                        .tooltip({
                            let location = chrome::location(document);
                            move |window, cx| Tooltip::new(location.clone()).build(window, cx)
                        })
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(chrome::file_mark(document.language.as_ref())),
                )
                .selected(id == active_id)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.activate_document_with_focus(id, window, cx);
                    cx.notify();
                }))
                .suffix(
                    div()
                        .h_flex()
                        .items_center()
                        .gap_1()
                        .when(document.is_dirty(), |row| {
                            row.child(div().text_color(cx.theme().muted_foreground).child("●"))
                        })
                        .child(
                            Button::new(format!("close-tab-{}", id.0))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .accessibility_label("关闭标签")
                                .tooltip("关闭标签")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.request_close_document(id, window, cx)
                                })),
                        ),
                )
        });
        TabBar::new("documents")
            .folder()
            .small()
            .max_width(px(220.))
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
            )
    }

    fn render_toolbar(
        &self,
        active: Option<&Document>,
        can_edit: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .h_flex()
            .h_8()
            .gap_2()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .px_3()
            .flex_shrink_0()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(active.map(chrome::location).unwrap_or_default()),
            )
            .child(
                Button::new("open-file")
                    .ghost()
                    .small()
                    .label("打开…")
                    .tooltip("打开文件")
                    .disabled(!can_edit)
                    .on_click(cx.listener(|this, _, window, cx| this.open(cx, window))),
            )
            .child(
                Button::new("find-in-document")
                    .ghost()
                    .small()
                    .icon(IconName::Search)
                    .accessibility_label("查找文档内容")
                    .tooltip("查找文档内容")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_search_panel(this.active_id(), false, window, cx)
                    })),
            )
            .when_some(
                // 只有支持预览的语言才显示这个按钮，避免给不可用的操作入口。
                active.filter(|document| editor::supports_preview(document.language.as_ref())),
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
    }

    fn render_editor(
        &self,
        active: Option<&Document>,
        can_edit: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .min_h_0()
            .child(if let Some(document) = active {
                let editor_panel = Editor::new(&document.editor)
                    .aria_label("文档正文")
                    .appearance(false)
                    .bordered(false)
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
            })
    }

    fn render_status(&self, active: Option<&Document>, cx: &App) -> impl IntoElement {
        let position = active.map(|document| document.editor.read(cx).cursor_position());
        StatusBar::new()
            .flex_shrink_0()
            .px_3()
            .when_some(active, |bar, document| {
                bar.right(self.render_encoding_menu(document, cx))
            })
            .right(active.map(chrome::metadata).unwrap_or_default())
            .left(if let Some(position) = position {
                format!("行 {}，列 {}", position.line + 1, position.character + 1)
            } else {
                "未命名".to_string()
            })
            .child(if self.loading {
                "处理中…".into()
            } else if self.message.is_empty()
                && active.is_some_and(|document| document.editor.read(cx).value().is_empty())
            {
                "直接输入，或拖入文件以打开".into()
            } else {
                self.message.clone()
            })
    }

    fn render_encoding_menu(&self, document: &Document, cx: &App) -> impl IntoElement {
        let current = document.encoding;
        let has_path = document.path.is_some();
        let action_context = document.editor.read(cx).focus_handle(cx);
        div().debug_selector(|| "document-encoding".into()).child(
            Button::new(("document-encoding", document.id.0))
                .label(current.label())
                .accessibility_label(format!("文档编码：{}", current.label()))
                .tooltip("更改文档编码")
                .ghost()
                .xsmall()
                .disabled(self.is_busy())
                .dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, window, cx| {
                    menu.action_context(action_context.clone())
                        .when(has_path, |menu| {
                            menu.submenu("以编码重新打开", window, cx, move |menu, _, _| {
                                encoding_items(menu, Some(current), |encoding| {
                                    Box::new(editor::ReopenWithEncoding(encoding))
                                })
                            })
                        })
                        .submenu("以编码保存", window, cx, move |menu, _, _| {
                            encoding_items(menu, Some(current), |encoding| {
                                Box::new(editor::SaveWithEncoding(encoding))
                            })
                        })
                        .separator()
                        .submenu("以编码打开", window, cx, |menu, _, _| {
                            encoding_items(menu, None, |encoding| {
                                Box::new(editor::OpenWithEncoding(encoding))
                            })
                        })
                }),
        )
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
                .v_flex()
                .gap_2()
                .child("Markdown 预览")
                .child("在左侧输入 Markdown，预览会同步更新。")
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

fn encoding_items(
    menu: PopupMenu,
    current: Option<Encoding>,
    action: impl Fn(Encoding) -> Box<dyn Action>,
) -> PopupMenu {
    let additional = current.filter(|encoding| !Encoding::OPTIONS.contains(encoding));
    Encoding::OPTIONS
        .iter()
        .copied()
        .chain(additional)
        .fold(menu, |menu, encoding| {
            menu.menu_with_check(
                encoding.label(),
                Some(encoding) == current,
                action(encoding),
            )
        })
}
