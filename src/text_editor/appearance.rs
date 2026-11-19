//! 工作区外观：颜色只在这里定义，视图统一读取语义主题。
use gpui_kit::component::theme::{Theme, ThemeConfig};
use gpui_kit::*;
use std::rc::Rc;

pub(super) fn init(cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new(editor_theme((*theme.light_theme).clone(), false));
    theme.dark_theme = Rc::new(editor_theme((*theme.dark_theme).clone(), true));
    Theme::sync_system_appearance(None, cx);
}

fn editor_theme(mut config: ThemeConfig, dark: bool) -> ThemeConfig {
    config.name = if dark {
        "Rueditor Dark"
    } else {
        "Rueditor Light"
    }
    .into();
    config.font_size = Some(14.);
    config.mono_font_size = Some(13.);
    config.radius = Some(4);
    config.shadow = Some(false);
    let (paper, chrome, foreground, muted, border, accent, selection) = if dark {
        (
            "#202127", "#282A31", "#DCE0E8", "#A0A7B5", "#363942", "#8BAAF0", "#354568",
        )
    } else {
        (
            "#FAFAFC", "#F0F1F5", "#303540", "#666E7D", "#DDE0E7", "#4269B2", "#DCE7FA",
        )
    };
    let colors = &mut config.colors;
    colors.background = Some(paper.into());
    colors.foreground = Some(foreground.into());
    colors.muted = Some(chrome.into());
    colors.muted_foreground = Some(muted.into());
    colors.border = Some(border.into());
    colors.primary = Some(accent.into());
    colors.caret = Some(accent.into());
    colors.selection = Some(selection.into());
    colors.tab = Some(chrome.into());
    colors.tab_bar = Some(chrome.into());
    colors.tab_active = Some(paper.into());
    colors.tab_foreground = Some(muted.into());
    colors.tab_active_foreground = Some(foreground.into());
    colors.title_bar = Some(chrome.into());
    colors.title_bar_border = Some(border.into());
    colors.status_bar = Some(chrome.into());
    colors.status_bar_border = Some(border.into());
    let highlight = config.highlight.get_or_insert_with(Default::default);
    let color = |hex: u32| rgb(hex).into();
    highlight.editor_background = Some(color(if dark { 0x202127 } else { 0xFAFAFC }));
    highlight.editor_foreground = Some(color(if dark { 0xDCE0E8 } else { 0x303540 }));
    highlight.editor_active_line = Some(color(if dark { 0x282B34 } else { 0xF0F1F6 }));
    highlight.editor_line_number = Some(color(if dark { 0x8B93A3 } else { 0x747C8A }));
    highlight.editor_active_line_number = highlight.editor_foreground;
    let style = |light: &str, dark_hex: &str| {
        serde_json::from_value(serde_json::json!({
            "color": if dark { dark_hex } else { light }
        }))
        .expect("valid syntax color")
    };
    let syntax = &mut highlight.syntax;
    syntax.keyword = Some(style("#873FA0", "#C9A0E5"));
    syntax.function = Some(style("#285EA8", "#8CB6ED"));
    syntax.constructor = Some(style("#76571D", "#E3C080"));
    syntax.type_ = Some(style("#76571D", "#E3C080"));
    syntax.string = Some(style("#3D6A36", "#ACC991"));
    syntax.string_escape = Some(style("#946027", "#E1B378"));
    syntax.string_regex = Some(style("#946027", "#E1B378"));
    syntax.string_special = syntax.string_escape;
    syntax.string_special_symbol = syntax.string_escape;
    syntax.comment = Some(style("#667080", "#969FAE"));
    syntax.comment_doc = syntax.comment;
    syntax.number = Some(style("#945226", "#DDA67A"));
    syntax.boolean = syntax.number;
    syntax.constant = syntax.number;
    syntax.property = Some(style("#825138", "#DAB18A"));
    syntax.attribute = syntax.property;
    syntax.variable = Some(style("#303540", "#DCE0E8"));
    syntax.variable_special = Some(style("#9A3E68", "#DCA1BF"));
    syntax.tag = Some(style("#285EA8", "#8CB6ED"));
    syntax.operator = Some(style("#4F5969", "#B1BBCB"));
    syntax.punctuation = syntax.operator;
    syntax.punctuation_bracket = syntax.operator;
    syntax.punctuation_delimiter = syntax.operator;
    syntax.punctuation_special = syntax.string_escape;
    syntax.embedded = syntax.variable;
    syntax.text_literal = syntax.variable;
    config
}

#[cfg(test)]
mod tests {
    use super::init;
    use gpui_kit::component::theme::{Theme, ThemeMode};
    use gpui_kit::{TestAppContext, px};

    #[gpui_kit::test]
    fn appearance_switch_preserves_editor_palette_and_type_scale(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            init(cx);
            for mode in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::Light] {
                Theme::change(mode, None, cx);
                let theme = Theme::global(cx);
                assert_eq!(theme.font_size, px(14.));
                assert_eq!(theme.mono_font_size, px(13.));
                assert_ne!(theme.background, theme.tab_bar);
                assert_eq!(theme.background, theme.tab_active);
                assert_ne!(theme.foreground, theme.background);
                assert_eq!(theme.mode, mode);
            }
        });
    }
    #[gpui_kit::test]
    fn syntax_roles_remain_readable_on_both_editor_surfaces(cx: &mut TestAppContext) {
        fn luminance(color: gpui_kit::Hsla) -> f32 {
            let color: gpui_kit::Rgba = color.into();
            let linear = |v: f32| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
        }
        cx.update(|cx| {
            gpui_kit::init(cx);
            init(cx);
            for mode in [ThemeMode::Light, ThemeMode::Dark] {
                Theme::change(mode, None, cx);
                let theme = Theme::global(cx);
                let syntax = &theme.highlight_theme.style.syntax;
                let paper = luminance(
                    theme
                        .highlight_theme
                        .style
                        .editor_background
                        .unwrap_or(theme.background),
                );
                for role in [
                    "keyword", "function", "string", "comment", "number", "property", "tag",
                    "type", "variable",
                ] {
                    let color = syntax
                        .style(role)
                        .and_then(|style| style.color)
                        .expect(role);
                    let ink = luminance(color);
                    let contrast = (ink.max(paper) + 0.05) / (ink.min(paper) + 0.05);
                    assert!(contrast >= 4.5, "{mode:?} {role} contrast {contrast}");
                }
                assert_ne!(
                    syntax.style("function").unwrap().color,
                    syntax.style("keyword").unwrap().color
                );
                assert_ne!(
                    syntax.style("property").unwrap().color,
                    syntax.style("variable").unwrap().color
                );
                assert_ne!(
                    syntax.style("string.escape").unwrap().color,
                    syntax.style("string").unwrap().color
                );
            }
        });
    }
}
