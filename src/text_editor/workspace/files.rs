// 文件打开、保存及关闭确认。
use super::super::{
    document::{Document, DocumentId, Encoding, Newline},
    file_io,
};
use super::Workspace;
use gpui_kit::component::input::FoldRange;
use gpui_kit::*;
use std::path::{Path, PathBuf};

/// 保存成功且文档不再 dirty 之后要继续做的事。
#[derive(Clone, Copy)]
pub(super) enum AfterSave {
    /// 保存的就是要关闭的那份文档时，接着移除标签。
    Close(DocumentId),
    /// 保存成功后继续询问下一个未保存文档。
    Quit,
}

impl Workspace {
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
    pub(super) fn open(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        self.open_with_encoding(None, window, cx);
    }

    pub(super) fn open_with_encoding(
        &mut self,
        encoding: Option<Encoding>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
            iter_and_load_files(paths, encoding, request_id, this, cx);
        })
        .detach();
        cx.notify();
    }

    pub(super) fn reopen_with_encoding(
        &mut self,
        id: DocumentId,
        encoding: Encoding,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_busy() {
            return;
        }
        let Some(document) = self.document(id) else {
            return;
        };
        let Some(path) = document.path.clone() else {
            self.message = "请先保存文档，再以编码重新打开".into();
            cx.notify();
            return;
        };
        let revision = document.revision;
        let dirty_title = document.is_dirty().then(|| document.title.clone());
        self.loading = true;
        let prompt = dirty_title.map(|title| {
            window.prompt(
                PromptLevel::Warning,
                "放弃修改并重新打开？",
                Some(&format!(
                    "将以 {} 重新打开「{title}」。成功后，未保存的修改将被丢弃。",
                    encoding.label()
                )),
                &[
                    PromptButton::cancel("取消"),
                    PromptButton::new("放弃修改并重新打开"),
                ],
                cx,
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            let confirmed = match prompt {
                Some(prompt) => matches!(prompt.await, Ok(1)),
                None => true,
            };
            let result = if confirmed {
                Some(
                    cx.background_spawn(async move {
                        file_io::load_with_encoding(&path, Some(encoding))
                    })
                    .await,
                )
            } else {
                None
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.loading = false;
                if let Some(result) = result {
                    match result {
                        Ok(loaded) => this.install_reopened(id, revision, loaded, window, cx),
                        Err(error) => this.message = format!("重新打开失败：{error}").into(),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn install_reopened(
        &mut self,
        id: DocumentId,
        revision: u64,
        loaded: file_io::LoadedText,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(document) = self.document_mut(id) else {
            return;
        };
        if document.revision != revision {
            self.message = "重新打开期间文档发生了变化，已保留当前内容".into();
            return;
        }
        document.encoding = loaded.encoding;
        document.newline = loaded.newline;
        document.disk_fingerprint = Some(loaded.fingerprint);
        document.revision += 1;
        document.saved_revision = document.revision;
        let yaml = document.language == "yaml";
        document.editor.update(cx, |state, cx| {
            state.set_value(loaded.text, window, cx);
            if yaml {
                let ranges = super::editor::yaml_fold_ranges(&state.value())
                    .into_iter()
                    .map(|(start, end)| FoldRange::new(start, end))
                    .collect();
                state.apply_highlighter_fold_candidates(ranges, cx);
            }
        });
        self.refresh_search(id, cx);
        if self.active_id() == id {
            self.focus_document(id, window, cx);
        }
        self.message = format!("已用 {} 重新打开", loaded.encoding.label()).into();
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
    pub(super) fn save(
        &mut self,
        id: DocumentId,
        save_as: bool,
        after: Option<AfterSave>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_document(id, save_as, after, None, window, cx);
    }

    pub(super) fn save_with_encoding(
        &mut self,
        id: DocumentId,
        encoding: Encoding,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_busy() {
            self.save_document(id, false, None, Some(encoding), window, cx);
        }
    }

    fn save_document(
        &mut self,
        id: DocumentId,
        save_as: bool,
        after: Option<AfterSave>,
        encoding: Option<Encoding>,
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
        let encoding = encoding.unwrap_or(document.encoding);
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
                            let bytes = file_io::bytes_for_save(&text, encoding, newline)?;
                            file_io::save_if_unchanged(&path, &bytes, expected.as_ref())
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
                    Some(Ok((path, fingerprint))) => {
                        let can_continue = if let Some(document) = this.document_mut(id) {
                            document.title = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                                .into();
                            document.path = Some(path);
                            document.encoding = encoding;
                            document.saved_revision = revision;
                            document.disk_fingerprint = Some(fingerprint);
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
    pub(super) fn request_close_document(
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
    pub(super) fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    /// 处理窗口关闭请求，完成确认后才真正移除窗口。
    pub(in crate::text_editor) fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_quit(window, cx);
    }

    /// 把拖入的文件按打开流程逐个加载。忙碌时忽略，避免和正在进行的加载交错。
    pub(super) fn drop_files(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_paths(paths.paths().to_vec(), window, cx);
    }

    /// 从启动参数、系统打开事件或窗口拖放接收文件路径并异步加载。
    pub(in crate::text_editor) fn open_paths(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_busy() {
            return;
        }
        if paths.is_empty() {
            return;
        }
        self.loading_request_id += 1;
        let request_id = self.loading_request_id;
        self.loading = true;
        self.pending_loads = paths.len();
        cx.spawn_in(window, async move |this, cx| {
            iter_and_load_files(paths, None, request_id, this, cx);
        })
        .detach();
        cx.notify();
    }
}

fn iter_and_load_files(
    paths: Vec<PathBuf>,
    encoding: Option<Encoding>,
    request_id: u64,
    this: WeakEntity<Workspace>,
    cx: &mut AsyncWindowContext,
) {
    for path in paths {
        let weak = this.clone();
        let task = cx.background_spawn(async move {
            let result = file_io::load_with_encoding(&path, encoding);
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
        })
        .detach();
    }
}
