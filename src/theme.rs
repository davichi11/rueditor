//! 产品主题定义：集中维护调色板，视图只读取主题中的语义角色。

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Window, px, rgb};

/// 切换基础主题后应用 Mori 配色，并同步到底层行为组件。
///
/// 启动时尚无窗口，传入 `None`；运行时切换可传入当前窗口。
/// 未覆盖的颜色沿用 GPUI Kit 对应明暗主题，避免残留上一次主题的值。
pub fn apply(dark: bool, window: Option<&mut Window>, cx: &mut App) {
    // 先恢复完整的基础主题，再覆盖产品需要定制的语义颜色。
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let theme = Theme::global_mut(cx);
    // Root 将基础字号映射为 rem；字号与圆角在此统一定义。
    theme.font_size = px(16.);
    theme.radius = px(8.);
    theme.radius_lg = px(16.);
    // 深色保持低亮度表面，浅色通过暖白与低饱和绿色区分内容和导航。
    if dark {
        theme.colors.background = rgb(0x1c201e).into();
        theme.colors.sidebar = rgb(0x171b19).into();
        theme.colors.primary = rgb(0x97c7ab).into();
        theme.colors.primary_foreground = rgb(0x172a20).into();
    } else {
        theme.colors.background = rgb(0xfdfdfb).into();
        theme.colors.foreground = rgb(0x292e2a).into();
        theme.colors.sidebar = rgb(0xf2f3ef).into();
        theme.colors.muted = rgb(0xf0f2ed).into();
        theme.colors.muted_foreground = rgb(0x747b73).into();
        theme.colors.border = rgb(0xe3e6df).into();
        theme.colors.primary = rgb(0x426b50).into();
        theme.colors.primary_foreground = rgb(0xffffff).into();
        theme.colors.primary_hover = rgb(0x365b42).into();
        theme.colors.primary_active = rgb(0x2c4b36).into();
        theme.colors.secondary = rgb(0xe8eee5).into();
        theme.colors.secondary_foreground = rgb(0x426b50).into();
    }
    // 直接修改全局主题后，同步滚动条等 Base 组件使用的主题投影。
    Theme::sync_base(cx);
}
