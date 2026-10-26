//! Rueditor 应用壳：平台无关的快捷键和窗口启动配置。

use super::editor::{self, FindNext, FindPrevious};
use gpui_kit::component::input::{Replace, Search};
use gpui_kit::*;

/// 窗口和平台集成使用的应用标识。
pub(super) const APP_ID: &str = "local.rueditor.app";

/// 返回使用平台主修饰键的应用级快捷键。
///
/// GPUI 的 `secondary` 在 macOS 上表示 Command，在 Windows/Linux 上表示
/// Control，避免把 macOS 专用的 `cmd` 绑定错误地带到其他平台。
pub(super) fn key_bindings() -> [KeyBinding; 18] {
    [
        KeyBinding::new("secondary-n", editor::New, None),
        KeyBinding::new("secondary-o", editor::Open, None),
        KeyBinding::new("secondary-s", editor::Save, None),
        KeyBinding::new("secondary-shift-s", editor::SaveAs, None),
        KeyBinding::new("secondary-w", editor::Close, None),
        KeyBinding::new("secondary-q", editor::Quit, None),
        KeyBinding::new("secondary-f", Search, None),
        KeyBinding::new("secondary-shift-f", Replace, None),
        KeyBinding::new("secondary-g", FindNext, None),
        KeyBinding::new("secondary-shift-g", FindPrevious, None),
        KeyBinding::new("secondary-l", editor::GotoLine, None),
        KeyBinding::new("secondary-d", editor::DuplicateLine, None),
        KeyBinding::new("secondary-shift-k", editor::DeleteLine, None),
        KeyBinding::new("alt-up", editor::MoveLineUp, None),
        KeyBinding::new("alt-down", editor::MoveLineDown, None),
        KeyBinding::new("secondary-[", editor::OutdentSelection, None),
        KeyBinding::new("secondary-]", editor::IndentSelection, None),
        KeyBinding::new("secondary-shift-p", editor::TogglePreview, None),
    ]
}

/// 构造 CotEditor 风格的应用级原生菜单栏。
pub(super) fn app_menus() -> [Menu; 8] {
    [
        Menu::new("文件").items([
            MenuItem::action("新建", editor::New),
            MenuItem::action("打开…", editor::Open),
            MenuItem::separator(),
            MenuItem::action("保存", editor::Save),
            MenuItem::action("另存为…", editor::SaveAs),
            MenuItem::separator(),
            MenuItem::action("关闭", editor::Close),
            MenuItem::action("退出", editor::Quit),
        ]),
        Menu::new("编辑").items([
            MenuItem::action("撤销", component::input::Undo),
            MenuItem::action("重做", component::input::Redo),
            MenuItem::separator(),
            MenuItem::action("剪切", component::input::Cut),
            MenuItem::action("复制", component::input::Copy),
            MenuItem::action("粘贴", component::input::Paste),
            MenuItem::action("全选", component::input::SelectAll),
            MenuItem::separator(),
            MenuItem::action("跳转到行…", editor::GotoLine),
            MenuItem::action("复制当前行", editor::DuplicateLine),
            MenuItem::action("删除当前行", editor::DeleteLine),
            MenuItem::action("上移当前行", editor::MoveLineUp),
            MenuItem::action("下移当前行", editor::MoveLineDown),
        ]),
        Menu::new("格式").items([
            MenuItem::action("格式化文档", editor::Format),
            MenuItem::action("增加缩进", editor::IndentSelection),
            MenuItem::action("减少缩进", editor::OutdentSelection),
            MenuItem::action("转为大写", editor::UppercaseSelection),
            MenuItem::action("转为小写", editor::LowercaseSelection),
            MenuItem::action("清理行尾空白", editor::TrimTrailingWhitespace),
        ]),
        Menu::new("显示").items([
            MenuItem::action("显示行号", editor::ToggleLineNumbers),
            MenuItem::action("自动换行", editor::ToggleSoftWrap),
            MenuItem::action("显示空白", editor::ToggleShowWhitespaces),
            MenuItem::action("智能缩进", editor::ToggleSmartIndent),
            MenuItem::action("自动闭合括号", editor::ToggleAutoClose),
            MenuItem::action("切换代码折叠", editor::ToggleFolding),
            MenuItem::separator(),
            MenuItem::action("Markdown 预览", editor::TogglePreview),
        ]),
        Menu::new("文本").items([
            MenuItem::submenu(Menu::new("语言").items([
                MenuItem::action("纯文本", editor::SetPlaintext),
                MenuItem::action("Rust", editor::SetRust),
                MenuItem::action("Python", editor::SetPython),
                MenuItem::action("JavaScript", editor::SetJavaScript),
                MenuItem::action("TypeScript", editor::SetTypeScript),
                MenuItem::action("TSX", editor::SetTsx),
                MenuItem::action("JSON", editor::SetJson),
                MenuItem::action("HTML", editor::SetHtml),
                MenuItem::action("CSS", editor::SetCss),
                MenuItem::action("Markdown", editor::SetMarkdown),
                MenuItem::action("TOML", editor::SetToml),
                MenuItem::action("YAML", editor::SetYaml),
            ])),
            MenuItem::separator(),
            MenuItem::submenu(Menu::new("Tab 宽度").items([
                MenuItem::action("2", editor::SetTabSize2),
                MenuItem::action("4", editor::SetTabSize4),
                MenuItem::action("8", editor::SetTabSize8),
            ])),
        ]),
        Menu::new("查找").items([
            MenuItem::action("查找", Search),
            MenuItem::action("替换", Replace),
            MenuItem::action("下一个匹配", FindNext),
            MenuItem::action("上一个匹配", FindPrevious),
            MenuItem::separator(),
            MenuItem::action("切换正则表达式", editor::ToggleRegex),
            MenuItem::action("区分大小写", editor::ToggleCaseSensitive),
            MenuItem::action("全词匹配", editor::ToggleWholeWord),
        ]),
        Menu::new("窗口").items([MenuItem::action("新建标签页", editor::New)]),
        Menu::new("帮助").items([MenuItem::action("关于 Rueditor", editor::ShowAbout)]),
    ]
}

/// 把应用菜单安装到当前 GPUI 应用上。
pub(super) fn set_app_menus(cx: &mut App) {
    cx.set_menus(app_menus());
}

/// 构造主窗口配置，保留各平台默认装饰和窗口行为。
pub(super) fn window_options(cx: &App) -> WindowOptions {
    // 原生窗口几何使用像素，正文与文档信息栏仍采用相对尺寸。
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
