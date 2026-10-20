//! 文档模型：保存文件元数据与编辑修订，不复制 EditorState 的文本。
use super::file_io::FileFingerprint;
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

/// 单个文档的显示和输入设置。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EditorSettings {
    pub(super) show_line_numbers: bool,
    pub(super) soft_wrap: bool,
    pub(super) show_whitespaces: bool,
    pub(super) smart_indent: bool,
    pub(super) auto_close: bool,
    pub(super) tab_size: usize,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            soft_wrap: false,
            show_whitespaces: false,
            smart_indent: true,
            auto_close: true,
            tab_size: 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SearchMode {
    PlainText,
    Regex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SearchOptions {
    pub(super) mode: SearchMode,
    pub(super) case_sensitive: bool,
    pub(super) whole_word: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            mode: SearchMode::PlainText,
            case_sensitive: false,
            whole_word: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SearchSession {
    pub(super) query: String,
    pub(super) replacement: String,
    pub(super) options: SearchOptions,
    pub(super) open: bool,
    pub(super) replace_mode: bool,
    pub(super) error: Option<String>,
}

/// 编辑器当前打开的单个文档及其编辑状态。
pub(super) struct Document {
    pub(super) id: DocumentId,
    pub(super) path: Option<PathBuf>,
    pub(super) title: SharedString,
    pub(super) editor: Entity<EditorState>,
    #[allow(dead_code)] // Consumed by the later per-document settings workflow.
    pub(super) settings: EditorSettings,
    pub(super) search: SearchSession,
    pub(super) language: SharedString,
    pub(super) folding_enabled: bool,
    pub(super) encoding: Encoding,
    pub(super) newline: Newline,
    pub(super) revision: u64,
    pub(super) saved_revision: u64,
    pub(super) disk_fingerprint: Option<FileFingerprint>,
}

impl Document {
    /// 判断内存内容是否已经偏离最近一次成功保存的版本。
    pub(super) fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }
}
