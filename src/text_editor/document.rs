//! 文档模型：保存文件元数据与编辑修订，不复制 EditorState 的文本。
use gpui_kit::component::input::EditorState;
use gpui_kit::{Entity, SharedString};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// 文档实体标识，用于异步文件操作完成后确认结果仍属于当前文档。
pub(super) struct DocumentId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 文档保存时使用的换行格式。
pub(super) enum Newline {
    Lf,
    Crlf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 文档的文本编码；当前支持 UTF-8 和 UTF-8 BOM。
pub(super) enum Encoding {
    Utf8,
    Utf8Bom,
}

/// 编辑器当前打开的单个文档及其编辑状态。
pub(super) struct Document {
    pub(super) id: DocumentId,
    pub(super) path: Option<PathBuf>,
    pub(super) title: SharedString,
    pub(super) editor: Entity<EditorState>,
    pub(super) language: SharedString,
    pub(super) folding_enabled: bool,
    pub(super) encoding: Encoding,
    pub(super) newline: Newline,
    pub(super) revision: u64,
    pub(super) saved_revision: u64,
}

impl Document {
    /// 判断内存内容是否已经偏离最近一次成功保存的版本。
    pub(super) fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }
}
