//! 工作台界面与会话状态。渲染只组合元素，状态变化集中在事件处理方法中。
//! 当前任务保存在内存中，不涉及模型调用、工具执行或持久化。

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Root, Selectable, Sizable, StyledExt, Theme,
    button::{Button, ButtonVariants},
    input::{InputEvent, Textarea, TextareaState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

// 由入口注册快捷键；新建和提交在工作台处理，退出由应用全局处理。
actions!(workspace, [NewTask, SubmitTask, Quit]);

/// 本次会话中创建的任务，使用稳定标识关联导航项和详情。
struct Task {
    /// 创建时分配的标识，不随列表展示顺序变化。
    id: u64,
    /// 去除首尾空白后的完整任务描述；侧栏仅显示简短摘要。
    prompt: SharedString,
}

/// 主窗口的状态所有者，跨渲染帧保留输入、选择与任务历史。
pub struct Workspace {
    /// 多行输入实体，保留文本、选区、撤销记录与输入焦点。
    composer: Entity<TextareaState>,
    /// 根区域焦点句柄，使子控件的命令可沿焦点路径传播到工作台。
    focus: FocusHandle,
    /// 按创建顺序保存任务，侧栏反向遍历以优先显示最近任务。
    tasks: Vec<Task>,
    /// 当前任务标识；None 表示新建任务首页。
    active: Option<u64>,
    /// 会话内单调递增的任务标识分配器。
    next_id: u64,
    /// 模板筛选标识，all 表示显示全部模板。
    category: &'static str,
    /// 持有输入事件订阅；句柄被释放时订阅也会取消。
    _subscription: Subscription,
}

/// 模板依次包含：稳定标识、图标、标题、说明、填入输入框的提示词。
/// 标识同时用于筛选和控件身份，不依赖显示文案或列表位置。
const TEMPLATES: [(&str, IconName, &str, &str, &str); 4] = [
    (
        "research",
        IconName::Globe,
        "调研与分析",
        "把零散信息，变成有用的洞察",
        "帮我调研一个行业，梳理市场趋势、主要竞争者和潜在机会。请先向我确认调研范围。",
    ),
    (
        "writing",
        IconName::FileText,
        "写作与表达",
        "从一个想法，到一份完整方案",
        "帮我起草一份项目方案，包括背景、目标、执行计划和预期成果。请先向我确认项目内容。",
    ),
    (
        "data",
        IconName::ChartNoAxesCombined,
        "数据与表格",
        "发现数字背后的线索",
        "帮我分析一份数据，识别关键指标、异常和变化趋势，并整理成清晰的结论。",
    ),
    (
        "planning",
        IconName::ListTodo,
        "计划与整理",
        "让复杂的工作，变得井井有条",
        "帮我把近期工作整理成可执行的计划，按优先级拆分步骤，并明确每一步的交付物。",
    ),
];

impl Workspace {
    /// 创建一次输入实体和订阅，并把初始焦点交给任务输入框。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("描述你的任务，剩下的交给 Mori…")
                .rows(3)
        });
        // 输入组件负责编辑行为；工作台监听变化以更新提交按钮的禁用状态。
        let subscription = cx.subscribe(&composer, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let focus = cx.focus_handle();
        composer.update(cx, |input, cx| input.focus(window, cx));
        Self {
            composer,
            focus,
            tasks: Vec::new(),
            active: None,
            next_id: 1,
            category: "all",
            _subscription: subscription,
        }
    }

    /// 回到首页并清空草稿，保留已创建的任务和当前模板分类。
    fn new_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active = None;
        self.composer.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    /// 将有效输入转为会话任务，切换到详情并清空输入框。
    /// 按钮和快捷键共用此入口，空白校验必须在这里再次执行。
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = self.composer.read(cx).value().trim().to_owned();
        if prompt.is_empty() {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task {
            id,
            prompt: prompt.into(),
        });
        self.active = Some(id);
        self.composer
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    /// 固定宽度的导航区：品牌、新建入口、独立滚动的历史列表和主题切换。
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .v_flex()
            .w_64()
            .flex_shrink_0()
            .h_full()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border)
            .p_4()
            .gap_6()
            .child(
                div()
                    .h_flex()
                    .gap_3()
                    .px_2()
                    .py_3()
                    .child(
                        div()
                            .size_8()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(theme.radius)
                            .bg(theme.primary)
                            .text_color(theme.primary_foreground)
                            .child(Icon::new(IconName::Sparkles).size_5()),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Mori"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("工作搭子"),
                    ),
            )
            .child(
                Button::new("new-task")
                    .outline()
                    .w_full()
                    .icon(IconName::Plus)
                    .label("新建任务")
                    .tooltip("新建任务 ⌘N")
                    .on_click(cx.listener(|this, _, window, cx| this.new_task(window, cx))),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(
                        Button::new("home")
                            .ghost()
                            .w_full()
                            .justify_start()
                            .icon(IconName::House)
                            .label("工作台")
                            .selected(self.active.is_none())
                            .on_click(cx.listener(|this, _, window, cx| this.new_task(window, cx))),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(Icon::new(IconName::Layers).size_4())
                            .child("个人工作空间"),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .px_3()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("最近任务")
                            .child(self.tasks.len().to_string()),
                    )
                    .child(
                        div()
                            .id("task-list")
                            .v_flex()
                            .gap_1()
                            .overflow_y_scroll()
                            .min_h_0()
                            .when(self.tasks.is_empty(), |el| {
                                el.child(
                                    div()
                                        .px_3()
                                        .py_4()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("从第一件小事开始。\n你的任务会出现在这里。"),
                                )
                            })
                            .children(self.tasks.iter().rev().map(|task| {
                                let id = task.id;
                                Button::new(("task", id))
                                    .ghost()
                                    .w_full()
                                    .justify_start()
                                    .icon(IconName::MessageSquare)
                                    .label(task.prompt.chars().take(13).collect::<String>())
                                    .tooltip(task.prompt.clone())
                                    .selected(self.active == Some(id))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.active = Some(id);
                                        cx.notify();
                                    }))
                            })),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .p_3()
                            .rounded(theme.radius)
                            .bg(theme.background)
                            .border_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .text_sm()
                                    .child(Icon::new(IconName::Leaf).size_4())
                                    .child("留一点时间给自己"),
                            )
                            .child(
                                div()
                                    .mt_2()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("让繁琐的工作，在这里变简单。"),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .px_2()
                            .child(
                                div()
                                    .size_8()
                                    .rounded(theme.radius_full())
                                    .bg(theme.secondary)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_sm()
                                    .child("我"),
                            )
                            .child(div().flex_1().text_sm().child("个人空间"))
                            .child(
                                Button::new("theme")
                                    .ghost()
                                    .small()
                                    .icon(if theme.is_dark() {
                                        IconName::Sun
                                    } else {
                                        IconName::Moon
                                    })
                                    .tooltip("切换浅色 / 深色主题")
                                    .accessibility_label("切换主题")
                                    .on_click(|_, window, cx| {
                                        crate::theme::apply(
                                            !Theme::global(cx).is_dark(),
                                            Some(window),
                                            cx,
                                        );
                                        window.refresh();
                                    }),
                            ),
                    ),
            )
    }

    /// 组合输入框和提交区；外层统一绘制边框，输入组件保留原生编辑行为。
    fn composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .v_flex()
            .w_full()
            .p_4()
            .gap_3()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                Textarea::new(&self.composer)
                    .aria_label("任务描述")
                    .appearance(false)
                    .bordered(false)
                    .h(rems(6.)),
            )
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(Icon::new(IconName::Sparkles).size_4())
                            .child("通用助手")
                            .child(div().ml_2().child("·"))
                            .child("界面预览"),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("⌘ ↵"),
                            )
                            .child(
                                Button::new("submit")
                                    .primary()
                                    .icon(IconName::ArrowUp)
                                    .label("创建任务")
                                    .disabled(self.composer.read(cx).value().trim().is_empty())
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.submit(window, cx)),
                                    ),
                            ),
                    ),
            )
    }

    /// 根据分类展示模板，卡片随可用宽度换行，点击后填入草稿而不直接提交。
    fn templates(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .v_flex()
            .gap_4()
            .w_full()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("从一个灵感开始"),
                    )
                    .child(div().h_flex().gap_1().children(
                        [("all", "全部"), ("writing", "写作"), ("data", "数据")].map(
                            |(id, label)| {
                                Button::new(SharedString::from(format!("category-{id}")))
                                    .ghost()
                                    .small()
                                    .label(label)
                                    .selected(self.category == id)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.category = id;
                                        cx.notify();
                                    }))
                            },
                        ),
                    )),
            )
            .child(
                div().flex().flex_wrap().gap_3().children(
                    TEMPLATES
                        .iter()
                        .filter(|(id, ..)| self.category == "all" || self.category == *id)
                        .map(|&(id, icon, title, description, prompt)| {
                            Button::new(SharedString::from(format!("template-{id}")))
                                .outline()
                                .h_auto()
                                .w(rems(21.))
                                .flex_grow(1.)
                                .p_4()
                                .justify_start()
                                .child(
                                    div()
                                        .v_flex()
                                        .gap_3()
                                        .w_full()
                                        .child(
                                            div()
                                                .h_flex()
                                                .gap_3()
                                                .child(
                                                    Icon::new(icon)
                                                        .size_5()
                                                        .text_color(theme.primary),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(title),
                                                )
                                                .child(
                                                    Icon::new(IconName::ArrowUpRight)
                                                        .size_4()
                                                        .text_color(theme.muted_foreground),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(description),
                                        ),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.composer.update(cx, |input, cx| {
                                        input.set_value(prompt, window, cx);
                                        input.focus(window, cx);
                                    });
                                    // set_value 不产生用户编辑事件，需主动刷新提交按钮状态。
                                    cx.notify();
                                }))
                        }),
                ),
            )
    }

    /// 首页按介绍、输入、模板排列；限制内容宽度以保持中文段落的可读性。
    fn home(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .v_flex()
            .gap_8()
            .w_full()
            .max_w(rems(46.))
            .py_12()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .text_color(theme.primary)
                            .text_sm()
                            .child(Icon::new(IconName::Sparkles).size_5())
                            .child("你的想法，值得被实现"),
                    )
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("今天，我们一起完成什么？"),
                    )
                    .child(
                        div().text_sm().text_color(theme.muted_foreground).child(
                            "整理信息、分析数据、写一份方案。把琐事交给我，专注你在意的事。",
                        ),
                    ),
            )
            .child(self.composer(cx))
            .child(self.templates(cx))
            .child(
                div()
                    .h_flex()
                    .justify_center()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(IconName::Command).size_3())
                    .child("N 新建任务")
                    .child(" · ")
                    .child("Enter 换行，⌘ Enter 创建任务"),
            )
    }

    /// 展示完整任务描述与原型能力边界，不生成虚构的执行状态或模型回复。
    fn task_detail(&self, task: &Task, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div().v_flex().w_full().max_w(rems(46.)).py_12().gap_6()
            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("任务 {:02} · 本次会话", task.id)))
            .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child("任务已准备好"))
            .child(div().p_6().bg(theme.muted).rounded(theme.radius_lg).child(task.prompt.clone()))
            .child(div().h_flex().items_start().gap_3()
                .child(Icon::new(IconName::Sparkles).size_5().text_color(theme.primary))
                .child(div().v_flex().gap_2()
                    .child("这件事，已经记下来了。")
                    .child(div().text_sm().text_color(theme.muted_foreground)
                        .child("当前为主界面预览，尚未连接智能体。任务仅保留在本次会话中，接入模型后可在这里查看执行过程与成果。"))))
            .child(Button::new("back-home").outline().label("继续创建任务")
                .on_click(cx.listener(|this, _, window, cx| this.new_task(window, cx))))
    }
}

impl Render for Workspace {
    /// 组合固定侧栏、顶部路径栏、可滚动内容和底部文案。
    /// 每帧复用已有实体，仅根据当前状态构建元素树。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("workspace")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .items_stretch()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &NewTask, window, cx| this.new_task(window, cx)))
            .on_action(cx.listener(|this, _: &SubmitTask, window, cx| this.submit(window, cx)))
            .child(self.sidebar(cx))
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        div()
                            .h_flex()
                            .h_16()
                            .flex_shrink_0()
                            .px_8()
                            .justify_between()
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .text_sm()
                                    .child("个人空间")
                                    .child(
                                        Icon::new(IconName::ChevronRight)
                                            .size_3()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(div().text_color(cx.theme().muted_foreground).child(
                                        if self.active.is_some() {
                                            "任务详情"
                                        } else {
                                            "工作台"
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Mori / 主界面预览"),
                            ),
                    )
                    .child(
                        div()
                            // 只有正文滚动；min_h_0 允许其收缩，避免挤出顶部和底部。
                            .id("main-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(div().v_flex().items_center().px_8().child(
                                if let Some(task) =
                                    self.tasks.iter().find(|t| Some(t.id) == self.active)
                                {
                                    self.task_detail(task, cx).into_any_element()
                                } else {
                                    self.home(cx).into_any_element()
                                },
                            )),
                    )
                    .child(
                        div()
                            .h_flex()
                            .justify_center()
                            .h_12()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("少一点忙碌，多一点创造。"),
                    ),
            )
            // Root 提供浮层设施，内容视图仍需各渲染一次浮层出口。
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
