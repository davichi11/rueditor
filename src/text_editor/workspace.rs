//! 多文档工作区：保留每个标签自己的编辑实体和文档元数据。

mod actions;
mod editing;
mod files;
mod search;
mod settings;
#[cfg(test)]
mod test_support;
mod view;

use super::{
    document::{Document, DocumentId, EditorSettings, Encoding, Newline},
    editor,
};
use gpui_kit::component::input::{EditorState, FoldRange, InputEvent, InputState};
use gpui_kit::*;
use settings::apply_editor_settings;
use std::{collections::HashSet, path::PathBuf};

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
        super::languages::init();
        let focus = cx.focus_handle();
        let search_query = cx.new(|cx| InputState::new(window, cx).placeholder("查找文档内容"));
        let search_replacement = cx.new(|cx| InputState::new(window, cx).placeholder("替换为"));
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
        workspace._subscriptions.push((
            DocumentId(0),
            cx.observe_window_appearance(window, |_, window, cx| {
                component::theme::Theme::sync_system_appearance(Some(window), cx);
            }),
        ));
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
        let language = editor::language_for_document(path.as_deref(), &text);
        let editor = cx.new(|cx| {
            let mut state = EditorState::new(window, cx)
                .language(language)
                // Search UI belongs to Workspace. Let shortcuts propagate to
                // its handlers instead of opening a second component panel.
                .searchable(false)
                .folding(true);
            state.set_highlighter_factory(super::syntax::factory(), cx);
            state.default_value(text)
        });
        let settings = EditorSettings::default();
        editor.update(cx, |state, cx| {
            apply_editor_settings(state, &settings, window, cx)
        });
        if language == "yaml" {
            editor.update(cx, |state, cx| {
                let candidates = editor::yaml_fold_ranges(&state.value())
                    .into_iter()
                    .map(|(start, end)| FoldRange::new(start, end))
                    .collect();
                state.apply_highlighter_fold_candidates(candidates, cx);
            });
        }
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
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let yaml_editor = this
                    .documents
                    .iter_mut()
                    .find(|document| document.id == subscribed_id)
                    .and_then(|document| {
                        document.revision += 1;
                        (document.language.as_ref() == "yaml").then(|| document.editor.clone())
                    });
                if let Some(editor) = yaml_editor {
                    editor.update(cx, |state, cx| {
                        let candidates = editor::yaml_fold_ranges(&state.value())
                            .into_iter()
                            .map(|(start, end)| FoldRange::new(start, end))
                            .collect();
                        state.apply_highlighter_fold_candidates(candidates, cx);
                    });
                }
                cx.notify();
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

    /// 按稳定 ID 查找文档。标签重排后下标会变，调用方不要缓存下标。
    fn document(&self, id: DocumentId) -> Option<&Document> {
        self.documents.iter().find(|document| document.id == id)
    }

    /// `document` 的可变版本。
    fn document_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.documents.iter_mut().find(|document| document.id == id)
    }

    /// 加载、确认框、保存或退出还没结束时，忽略会改文档集合的新操作。
    fn is_busy(&self) -> bool {
        self.loading || self.prompting || !self.saving.is_empty() || self.quitting
    }

    /// 当前标签的文档 ID。渲染和动作处理都从这里取，避免中途读到旧下标。
    fn active_id(&self) -> DocumentId {
        self.active_document_id
    }
}
