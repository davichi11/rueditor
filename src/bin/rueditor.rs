//! 文本编辑器可执行入口，具体实现位于 text_editor 模块。

#[path = "../text_editor/mod.rs"]
mod text_editor;

fn main() {
    text_editor::run();
}
