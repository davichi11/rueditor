//! 文本编辑器可执行入口，具体实现位于 text_editor 模块。

#[path = "../text_editor/mod.rs"]
mod text_editor;

/// 进程入口，只负责转到编辑器模块。
fn main() {
    text_editor::run();
}
