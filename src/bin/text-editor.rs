//! CotEditor 风格的第一阶段编辑器验证窗口。
//!
//! 这一阶段只验证单文档编辑能力：代码高亮、折叠、行号、撤销和中文输入。
//! 文件读写、多标签和查找替换留在后续阶段，避免把编辑器核心验证和工作流耦合。

use gpui_kit::component::{
    ActiveTheme, Root, StyledExt,
    input::{Editor, EditorState},
};
use gpui_kit::*;

const SAMPLE_RUST: &str = r#"// Mori text editor preview
fn main() {
    let message = "Hello from GPUI";
    println!("{message}");
    if message.len() > 0 {
        println!("syntax highlighting and folding");
    }
}
"#;

struct TextEditor {
    /// 编辑状态由 GPUI Entity 跨帧持有，Editor 只负责呈现和交互。
    editor: Entity<EditorState>,
    /// 根焦点句柄让编辑器能够接收键盘输入与命令。
    focus: FocusHandle,
}

impl TextEditor {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("rust")
                .default_value(SAMPLE_RUST)
        });
        let focus = cx.focus_handle();
        editor.update(cx, |editor, cx| editor.focus(window, cx));
        Self { editor, focus }
    }
}

impl Render for TextEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("text-editor")
            .track_focus(&self.focus)
            .size_full()
            .v_flex()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            // 保持编辑器占满剩余高度，避免窗口缩小时正文被工具栏挤出。
            .child(
                div()
                    .h_flex()
                    .h_10()
                    .flex_shrink_0()
                    .px_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child("未命名 · Rust")
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("高亮 · 折叠"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(Editor::new(&self.editor).h(relative(1.))),
            )
            .child(
                div()
                    .h_flex()
                    .h_8()
                    .flex_shrink_0()
                    .px_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Ln 1, Col 1")
                    .child(div().flex_1())
                    .child("UTF-8 · LF · 8 行"),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

fn main() {
    application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.spawn(async move |cx| {
                cx.open_window(WindowOptions::default(), |window, cx| {
                    let editor = cx.new(|cx| TextEditor::new(window, cx));
                    cx.new(|cx| Root::new(editor, window, cx))
                })
                .expect("无法打开文本编辑器");
            })
            .detach();
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn sample_contains_foldable_rust_block() {
        assert!(super::SAMPLE_RUST.contains("fn main()"));
        assert!(super::SAMPLE_RUST.matches('{').count() >= 2);
        assert!(super::SAMPLE_RUST.contains("println!"));
    }
}
