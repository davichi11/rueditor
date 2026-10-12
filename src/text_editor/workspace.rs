//! 多文档工作区：保留每个标签自己的编辑实体和文档元数据。

use super::document::{Document, DocumentId, Encoding, Newline};
use super::{editor, file_io};
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    ActiveTheme, Disableable, Root, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Editor, EditorState, InputEvent},
    status_bar::StatusBar,
    tab::{Tab, TabBar},
};
use gpui_kit::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use editor::{Close, Format, New, Open, Save, SaveAs};

#[derive(Clone, Copy)]
enum AfterSave {
    Close(DocumentId),
    Quit,
}

/// 主窗口中的多文档状态。
pub(super) struct Workspace {
    documents: Vec<Document>,
    active_document_id: DocumentId,
    next_document_id: u64,
    focus: FocusHandle,
    loading_request_id: u64,
    pending_loads: usize,
    loading: bool,
    prompting: bool,
    saving: HashSet<DocumentId>,
    quitting: bool,
    quit_discarded: HashSet<DocumentId>,
    message: SharedString,
    _subscriptions: Vec<(DocumentId, Subscription)>,
}

impl Workspace {
    /// 创建包含一个未命名文档的新工作区。
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
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
            _subscriptions: Vec::new(),
        };
        let document =
            workspace.create_document(None, String::new(), Encoding::Utf8, Newline::Lf, window, cx);
        workspace.active_document_id = document.id;
        workspace.documents.push(document);
        workspace
    }

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
                .default_value(text)
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
            encoding,
            newline,
            revision: 0,
            saved_revision: 0,
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

    fn activate_document_with_focus(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.activate_document(id) {
            return false;
        }
        self.focus_document(id, window, cx);
        true
    }

    fn focus_document(&self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = self.document(id).map(|document| document.editor.clone()) {
            editor.update(cx, |state, cx| state.focus(window, cx));
        }
    }

    /// 当前文档的稳定 ID。
    #[cfg(test)]
    pub(super) fn active_document_id(&self) -> DocumentId {
        self.active_document_id
    }

    fn document(&self, id: DocumentId) -> Option<&Document> {
        self.documents.iter().find(|document| document.id == id)
    }

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
    pub(crate) fn format_document_for_test(
        &mut self,
        id: DocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.format_document(id, window, cx);
    }

    fn is_busy(&self) -> bool {
        self.loading || self.prompting || !self.saving.is_empty() || self.quitting
    }

    fn active_id(&self) -> DocumentId {
        self.active_document_id
    }

    fn path_matches(left: &Path, right: &Path) -> bool {
        if left == right {
            return true;
        }
        match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
            (Ok(left), Ok(right)) => left == right,
            _ => false,
        }
    }

    fn find_path(&self, path: &Path) -> Option<DocumentId> {
        self.documents.iter().find_map(|document| {
            document
                .path
                .as_deref()
                .filter(|document_path| Self::path_matches(document_path, path))
                .map(|_| document.id)
        })
    }

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
        let document = self.create_document(
            Some(loaded.path),
            loaded.text,
            loaded.encoding,
            loaded.newline,
            window,
            cx,
        );
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
            for path in paths {
                let weak = this.clone();
                let task = cx.background_spawn(async move {
                    let result = file_io::load(&path);
                    (path, result)
                });
                cx.spawn(async move |cx| {
                    let (path, result) = task.await;
                    let _ = weak.update_in(cx, |this, window, cx| {
                        if this.loading_request_id != request_id {
                            return;
                        }
                        this.install_one_result(path, result, window, cx);
                        this.pending_loads = this.pending_loads.saturating_sub(1);
                        this.loading = this.pending_loads != 0;
                        cx.notify();
                    });
                })
                .detach();
            }
        })
        .detach();
        cx.notify();
    }

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
                        cx.background_spawn(async move {
                            file_io::save(&path, &file_io::bytes_for_save(&text, encoding, newline))
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

    fn remove_document(&mut self, id: DocumentId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.documents.iter().position(|document| document.id == id) else {
            return;
        };
        self._subscriptions
            .retain(|(document_id, _)| *document_id != id);
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

    fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || !self.saving.is_empty() || self.prompting || self.quitting {
            return;
        }
        self.quit_discarded.clear();
        self.quitting = true;
        self.continue_quit(window, cx);
    }

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
            for path in paths {
                let weak = this.clone();
                let task = cx.background_spawn(async move {
                    let result = file_io::load(&path);
                    (path, result)
                });
                cx.spawn(async move |cx| {
                    let (path, result) = task.await;
                    let _ = weak.update_in(cx, |this, window, cx| {
                        if this.loading_request_id != request_id {
                            return;
                        }
                        this.install_one_result(path, result, window, cx);
                        this.pending_loads = this.pending_loads.saturating_sub(1);
                        this.loading = this.pending_loads != 0;
                        cx.notify();
                    });
                })
                .detach();
            }
        })
        .detach();
        cx.notify();
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let language = active
            .and_then(|document| document.path.as_deref())
            .map(editor::language_for_path)
            .unwrap_or("plaintext");
        let encoding = active
            .map(|document| format!("{:?}", document.encoding))
            .unwrap_or_else(|| "Utf8".to_string());
        let newline = active
            .map(|document| format!("{:?}", document.newline))
            .unwrap_or_else(|| "Lf".to_string());
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
                        .label("×")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.request_close_document(id, window, cx)
                        })),
                )
        });
        let can_edit = !self.is_busy();
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
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .h_10()
                    .px_4()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("new")
                            .ghost()
                            .small()
                            .label("新建")
                            .disabled(!can_edit)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_tab(window, cx);
                            })),
                    )
                    .child(
                        Button::new("open")
                            .ghost()
                            .small()
                            .label("打开…")
                            .disabled(!can_edit)
                            .on_click(cx.listener(|this, _, window, cx| this.open(cx, window))),
                    )
                    .child(
                        Button::new("save")
                            .ghost()
                            .small()
                            .label("保存")
                            .disabled(!can_edit)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save(this.active_id(), false, None, window, cx)
                            })),
                    )
                    .child(
                        Button::new("save-as")
                            .ghost()
                            .small()
                            .label("另存为…")
                            .disabled(!can_edit)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save(this.active_id(), true, None, window, cx)
                            })),
                    )
                    .child(
                        Button::new("format")
                            .ghost()
                            .small()
                            .label("格式化")
                            .disabled(!can_edit)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.format_document(this.active_id(), window, cx)
                            })),
                    )
                    .child(div().flex_1())
                    .child(format!("{}{}", title, if active_dirty { " *" } else { "" })),
            )
            .child(
                TabBar::new("documents")
                    .underline()
                    .small()
                    .selected_index(active_index)
                    .children(tabs)
                    .suffix(Button::new("new-tab").ghost().small().label("+").on_click(
                        cx.listener(|this, _, window, cx| {
                            this.new_tab(window, cx);
                        }),
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(if let Some(document) = active {
                        Editor::new(&document.editor)
                            .aria_label("文档正文")
                            .readonly(!can_edit)
                            .h(relative(1.))
                            .into_any_element()
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
                    .right(format!("{encoding} · {newline} · {language}"))
                    .child(if self.loading {
                        "处理中…".into()
                    } else {
                        self.message.clone()
                    }),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
