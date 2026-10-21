//! 文档模型：保存文件元数据与编辑修订，不复制 EditorState 的文本。
use super::file_io::FileFingerprint;
use gpui_kit::component::input::EditorState;
use gpui_kit::{Entity, SharedString};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// 文档实体标识，用于异步文件操作完成后确认结果仍属于当前文档。
pub(super) struct DocumentId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 文档保存时使用的换行格式。编辑器内存里一律是 LF。
pub(super) enum Newline {
    /// Unix 换行。读取时的默认值，也是编辑器内部格式。
    Lf,
    /// Windows 换行。只在写入磁盘时把内部 LF 转成 CRLF。
    Crlf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 文档的文本编码；当前支持 UTF-8 和 UTF-8 BOM。
pub(super) enum Encoding {
    /// 无 BOM 的 UTF-8。
    Utf8,
    /// 以 EF BB BF 开头的 UTF-8；正文解码时会去掉这三字节。
    Utf8Bom,
}

/// 单个文档的显示和输入设置。修改这些项不会推进文档修订。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EditorSettings {
    /// 是否在编辑器左侧显示行号。
    pub(super) show_line_numbers: bool,
    /// 是否按窗口宽度软换行。
    pub(super) soft_wrap: bool,
    /// 是否把空格和制表符画出来。
    pub(super) show_whitespaces: bool,
    /// 换行时是否延续上一行的缩进。
    pub(super) smart_indent: bool,
    /// 输入左括号时是否自动补上右括号。
    pub(super) auto_close: bool,
    /// 一个 Tab 对应的空格数。当前始终使用空格，不插入硬 Tab。
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
/// 查找和替换的匹配方式。
pub(super) enum SearchMode {
    /// 按字面量匹配。用户输入会先转义，替换文本里的 `$` 保持原样。
    PlainText,
    /// 按正则表达式匹配。替换文本可以展开 `$1` 这类捕获组。
    Regex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 一次查找或替换使用的匹配选项，跟随当前文档的搜索会话。
pub(super) struct SearchOptions {
    pub(super) mode: SearchMode,
    /// 为 true 时区分大小写。默认不区分。
    pub(super) case_sensitive: bool,
    /// 为 true 时，匹配两端都不能再紧挨着 Unicode 词字符。
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
/// 绑定在单个文档上的查找/替换会话。
///
/// 查询、替换文本、选项、面板开关和错误都属于这份会话。
/// 工作区只保留一套输入框，切换标签时从这里恢复。
pub(super) struct SearchSession {
    pub(super) query: String,
    pub(super) replacement: String,
    pub(super) options: SearchOptions,
    /// 工作区查找面板是否展开。
    pub(super) open: bool,
    /// 面板是否同时显示替换行。
    pub(super) replace_mode: bool,
    /// 最近一次可恢复的查找或替换错误。成功匹配后清空。
    pub(super) error: Option<String>,
}

/// 编辑器当前打开的单个文档及其编辑状态。
pub(super) struct Document {
    pub(super) id: DocumentId,
    /// 磁盘路径。未命名文档为 `None`。
    pub(super) path: Option<PathBuf>,
    /// 标签和标题栏使用的文件名；未命名时为“未命名”。
    pub(super) title: SharedString,
    /// 这个标签独占的编辑器实体。正文只放在这里，不在文档里再复制一份。
    pub(super) editor: Entity<EditorState>,
    #[allow(dead_code)] // Consumed by the later per-document settings workflow.
    pub(super) settings: EditorSettings,
    pub(super) search: SearchSession,
    /// Tree-sitter 语言标识，例如 `rust`、`plaintext`。
    pub(super) language: SharedString,
    /// 是否启用代码折叠。切换它不会把文档标成未保存。
    pub(super) folding_enabled: bool,
    pub(super) encoding: Encoding,
    pub(super) newline: Newline,
    /// 文本每次变更加一。光标移动和设置切换不计入。
    pub(super) revision: u64,
    /// 最近一次成功保存时记下的 `revision`。
    pub(super) saved_revision: u64,
    /// 打开或保存成功时的磁盘快照，用来拒绝覆盖外部修改。
    pub(super) disk_fingerprint: Option<FileFingerprint>,
}

impl Document {
    /// 判断内存内容是否已经偏离最近一次成功保存的版本。
    pub(super) fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }
}
