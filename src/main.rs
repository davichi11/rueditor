//! Mori 桌面应用入口：初始化组件、注册命令并创建唯一的主窗口。

mod theme;
mod workspace;

use gpui_kit::component::Root;
use gpui_kit::*;

/// 启动事件循环；工作台的会话状态由窗口内的 Workspace 实体持有。
fn main() {
    application()
        // 为组件和工作台提供内置 SVG 图标资源。
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            // 必须先初始化组件设施，再设置主题或创建输入框等实体。
            gpui_kit::init(cx);
            theme::apply(false, None, cx);
            // 快捷键派发语义命令，与按钮共用工作台中的处理方法。
            cx.bind_keys([
                KeyBinding::new("cmd-n", workspace::NewTask, None),
                KeyBinding::new("cmd-enter", workspace::SubmitTask, None),
                KeyBinding::new("cmd-q", workspace::Quit, None),
            ]);
            cx.on_action(|_: &workspace::Quit, cx| cx.quit());
            // 居中计算依赖同步 App 上下文，因此在进入异步任务之前完成。
            let bounds = Bounds::centered(None, size(px(1220.), px(820.)), cx);
            cx.spawn(async move |cx| {
                cx.open_window(
                    WindowOptions {
                        // 原生窗口尺寸使用物理布局单位 px；内部排版使用 rem 比例。
                        // 最小尺寸为固定侧栏和任务输入区保留可用空间。
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(940.), px(680.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Mori · 智能体工作台".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    |window, cx| {
                        let view = cx.new(|cx| workspace::Workspace::new(window, cx));
                        // 每个窗口只包一层 Root，统一管理焦点、主题及浮层设施。
                        cx.new(|cx| Root::new(view, window, cx))
                    },
                )
                .expect("无法打开智能体工作台");
            })
            // 创建窗口的任务脱离局部句柄后继续执行。
            .detach();
            cx.activate(true);
        });
}
