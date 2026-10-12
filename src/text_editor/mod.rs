//! 编辑器初始化和窗口生命周期。

mod document;
mod editor;
mod file_io;
#[cfg(test)]
mod sample;
mod shell;
mod workspace;

use gpui_kit::component::Root;
use gpui_kit::*;
use workspace::Workspace;

/// 启动文本编辑器主窗口。
pub(crate) fn run() {
    application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.bind_keys(shell::key_bindings());
            let window_options = shell::window_options(cx);
            cx.spawn(async move |cx| {
                cx.open_window(window_options, |window, cx| {
                    let workspace = cx.new(|cx| Workspace::new(window, cx));
                    let weak = workspace.downgrade();
                    window.on_window_should_close(cx, move |window, cx| {
                        let _ = weak.update(cx, |workspace, cx| workspace.close(window, cx));
                        false
                    });
                    cx.new(|cx| Root::new(workspace, window, cx))
                })
                .expect("无法打开文本编辑器");
            })
            .detach();
            cx.activate(true);
        });
}

#[cfg(test)]
mod shell_tests {
    #[test]
    fn primary_application_shortcuts_use_secondary_modifier() {
        let bindings = super::shell::key_bindings();

        assert_eq!(bindings.len(), 6);
        assert!(bindings.iter().all(|binding| {
            binding
                .keystrokes()
                .first()
                .is_some_and(|keystroke| keystroke.inner().modifiers.secondary())
        }));
    }
}
