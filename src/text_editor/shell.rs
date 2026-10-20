//! Rueditor 应用壳：平台无关的快捷键和窗口启动配置。

use super::editor::{self, FindNext, FindPrevious};
use gpui_kit::component::input::{Replace, Search};
use gpui_kit::*;

pub(super) const APP_ID: &str = "local.rueditor.app";

/// 返回使用平台主修饰键的应用级快捷键。
///
/// GPUI 的 `secondary` 在 macOS 上表示 Command，在 Windows/Linux 上表示
/// Control，避免把 macOS 专用的 `cmd` 绑定错误地带到其他平台。
pub(super) fn key_bindings() -> [KeyBinding; 10] {
    [
        KeyBinding::new("secondary-n", editor::New, None),
        KeyBinding::new("secondary-o", editor::Open, None),
        KeyBinding::new("secondary-s", editor::Save, None),
        KeyBinding::new("secondary-shift-s", editor::SaveAs, None),
        KeyBinding::new("secondary-w", editor::Close, None),
        KeyBinding::new("secondary-q", editor::Close, None),
        KeyBinding::new("secondary-f", Search, None),
        KeyBinding::new("secondary-shift-f", Replace, None),
        KeyBinding::new("secondary-g", FindNext, None),
        KeyBinding::new("secondary-shift-g", FindPrevious, None),
    ]
}

/// 构造主窗口配置，保留各平台默认装饰和窗口行为。
pub(super) fn window_options(cx: &App) -> WindowOptions {
    // 原生窗口几何使用像素，正文与工具栏仍采用相对尺寸。
    let bounds = Bounds::centered(None, size(px(1000.), px(720.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(720.), px(480.))),
        titlebar: Some(TitlebarOptions {
            title: Some("Rueditor".into()),
            ..Default::default()
        }),
        app_id: Some(APP_ID.to_owned()),
        ..Default::default()
    }
}
