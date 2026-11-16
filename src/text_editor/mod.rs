//! 编辑器初始化和窗口生命周期。

mod appearance;
mod chrome;
mod document;
mod editor;
mod file_io;
#[cfg(test)]
mod sample;
mod shell;
mod workspace;

use gpui_kit::component::Root;
use gpui_kit::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use workspace::Workspace;

type OpenTarget = Rc<RefCell<Option<(AsyncApp, WeakEntity<Workspace>)>>>;

/// 启动文本编辑器主窗口。
pub(crate) fn run() {
    let open_target: OpenTarget = Rc::new(RefCell::new(None));
    let pending_paths = Rc::new(RefCell::new(Vec::<PathBuf>::new()));
    let target_for_urls = open_target.clone();
    let pending_for_urls = pending_paths.clone();
    let app = application().with_assets(assets::Assets);
    app.on_open_urls(move |urls| {
        let paths = editor::paths_from_open_urls(urls);
        if paths.is_empty() {
            return;
        }
        if let Some((app, workspace)) = target_for_urls.borrow().clone() {
            app.update(|cx| {
                let _ = workspace.update_in(cx, |workspace, window, cx| {
                    workspace.open_paths(paths, window, cx);
                });
            });
        } else {
            pending_for_urls.borrow_mut().extend(paths);
        }
    });
    app.run(move |cx| {
        init(cx);
        appearance::init(cx);
        cx.bind_keys(shell::key_bindings());
        shell::set_app_menus(cx);
        let window_options = shell::window_options(cx);
        let startup_paths = std::env::args_os()
            .skip(1)
            .map(PathBuf::from)
            .filter(|path| path.exists())
            .collect::<Vec<_>>();
        let open_target = open_target.clone();
        let pending_paths = pending_paths.clone();
        let app = cx.to_async();
        cx.spawn(async move |cx| {
            cx.open_window(window_options, move |window, cx| {
                let workspace = cx.new(|cx| Workspace::new(window, cx));
                *open_target.borrow_mut() = Some((app, workspace.downgrade()));
                let mut paths = std::mem::take(&mut *pending_paths.borrow_mut());
                paths.extend(startup_paths);
                workspace.update(cx, |workspace, cx| {
                    workspace.open_paths(paths, window, cx);
                });
                let weak = workspace.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    let _ = weak.update(cx, |workspace, cx| workspace.close(window, cx));
                    // 返回 false，关闭先交给工作区做保存确认，确认完成后再移除窗口。
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
    fn app_menu_groups_follow_coteditor_information_architecture() {
        let names = super::shell::app_menus()
            .into_iter()
            .map(|menu| menu.name.to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            [
                "文件", "编辑", "格式", "显示", "文本", "查找", "窗口", "帮助"
            ]
        );
    }

    #[test]
    fn primary_application_shortcuts_use_secondary_modifier() {
        let bindings = super::shell::key_bindings();

        assert_eq!(bindings.len(), 18);
        assert!(bindings[..10].iter().all(|binding| {
            binding
                .keystrokes()
                .first()
                .is_some_and(|keystroke| keystroke.inner().modifiers.secondary())
        }));
        for key in ["l", "d"] {
            assert!(
                bindings.iter().any(
                    |binding| binding.keystrokes().first().is_some_and(|stroke| {
                        stroke.inner().modifiers.secondary() && stroke.inner().key == key
                    })
                )
            );
        }
    }
}
