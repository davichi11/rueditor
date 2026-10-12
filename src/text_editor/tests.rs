//! 文件边界测试；后台打开使用平台选择器的测试模拟接口。

use super::super::document::{Encoding, Newline};
use super::super::editor::{JsonFormatResult, Open, SaveAs, format_json};
use super::super::file_io::{bytes_for_save, load};
use super::super::workspace::Workspace;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};
use std::io::ErrorKind;

#[test]
fn utf8_bom_and_crlf_round_trip() {
    let bytes = bytes_for_save("第一行\n第二行", Encoding::Utf8Bom, Newline::Crlf);
    assert_eq!(&bytes[..3], &[0xef, 0xbb, 0xbf]);
    assert!(bytes.windows(2).any(|pair| pair == b"\r\n"));
}

#[test]
fn loads_bom_and_crlf_as_normalized_editor_text() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"\xEF\xBB\xBFfirst\r\nsecond").unwrap();

    let loaded = load(file.path()).unwrap();

    assert_eq!(loaded.text, "first\nsecond");
    assert_eq!(loaded.encoding, Encoding::Utf8Bom);
    assert_eq!(loaded.newline, Newline::Crlf);
}

#[test]
fn rejects_mixed_and_standalone_cr_newlines() {
    for contents in [b"first\r\nsecond\nthird".as_slice(), b"first\rsecond"] {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), contents).unwrap();

        let error = load(file.path()).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert!(error.to_string().contains("混合换行"));
    }
}

#[test]
fn atomically_saves_new_file_with_requested_format() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("new.txt");

    let saved_path = super::super::file_io::save(
        &path,
        &bytes_for_save("first\nsecond", Encoding::Utf8Bom, Newline::Crlf),
    )
    .unwrap();

    assert_eq!(saved_path, path);
    assert_eq!(std::fs::read(path).unwrap(), b"\xEF\xBB\xBFfirst\r\nsecond");
}

#[test]
fn invalid_utf8_is_rejected_without_replacement() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), [0xff, 0xfe]).unwrap();
    let result = load(file.path());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidData);
}

#[test]
fn formats_valid_json_with_indentation() {
    let result = format_json(r#"{"name":"rueditor","enabled":true}"#).unwrap();
    assert_eq!(
        result,
        JsonFormatResult::Formatted(
            "{\n  \"name\": \"rueditor\",\n  \"enabled\": true\n}\n".into()
        )
    );
}

#[test]
fn repairs_json_before_formatting() {
    let result = format_json("{name: 'rueditor', enabled: true}").unwrap();
    assert!(
        matches!(result, JsonFormatResult::Repaired(value) if value.contains("\"name\": \"rueditor\""))
    );
}

#[test]
fn reports_unrepairable_json_without_result() {
    let error = format_json("").unwrap_err();
    assert!(error.contains("JSON"));
}

#[gpui_kit::test]
async fn workspace_keeps_document_editors_and_dirty_state_independent(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();

    let (first, second) = cx
        .update_window(handle.into(), |_, window, cx| {
            view.update(cx, |workspace, cx| {
                let first = workspace.active_document_id();
                let second = workspace.new_tab(window, cx);
                workspace.set_document_value(first, "first", window, cx);
                workspace.set_document_value(second, "second", window, cx);
                workspace.activate_document(first);
                (first, second)
            })
        })
        .unwrap();

    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.active_document_id(), first);
        assert_eq!(
            workspace.document_text(first, cx),
            Some("first".to_string())
        );
        assert_eq!(
            workspace.document_text(second, cx),
            Some("second".to_string())
        );
        assert!(workspace.document_is_dirty(first));
        assert!(workspace.document_is_dirty(second));
    });
}

#[gpui_kit::test]
async fn opens_selected_crlf_file_into_current_document(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "第一行\r\n第二行".as_bytes()).unwrap();
    let path = file.path().to_path_buf();
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.dispatch_action(Box::new(Open), cx);
    })
    .unwrap();
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(move |_| Some(vec![path]));
    cx.run_until_parked();

    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(
            workspace.document_text(id, cx),
            Some("第一行\n第二行".to_string())
        );
        assert_eq!(
            workspace.active_document_path(),
            Some(file.path().to_path_buf())
        );
        assert!(!workspace.document_is_dirty(id));
    });
}

#[gpui_kit::test]
async fn reopening_an_open_path_reuses_the_existing_tab(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "same file").unwrap();
    let path = file.path().to_path_buf();
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();

    for _ in 0..2 {
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.dispatch_action(Box::new(Open), cx);
        })
        .unwrap();
        let selected = path.clone();
        cx.simulate_path_prompt_response(move |_| Some(vec![selected]));
        cx.run_until_parked();
    }

    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.document_count(), 2);
        assert_eq!(workspace.active_document_path(), Some(path));
    });
}

#[gpui_kit::test]
async fn opening_multiple_files_installs_each_file_as_a_tab(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let first = tempfile::NamedTempFile::new().unwrap();
    let second = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(first.path(), "first").unwrap();
    std::fs::write(second.path(), "second").unwrap();
    let first_path = first.path().to_path_buf();
    let second_path = second.path().to_path_buf();
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.dispatch_action(Box::new(Open), cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(move |_| Some(vec![first_path, second_path]));
    cx.run_until_parked();

    cx.update(|cx| assert_eq!(view.read(cx).document_count(), 3));
}

#[gpui_kit::test]
async fn formatting_marks_a_clean_document_dirty(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            let id = workspace.active_document_id();
            workspace.set_document_value(id, "line  \n", window, cx);
            workspace.mark_document_saved(id);
            workspace.format_document_for_test(id, window, cx);
        });
    })
    .unwrap();

    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(workspace.document_text(id, cx), Some("line\n".to_string()));
        assert!(workspace.document_is_dirty(id));
    });
}

#[gpui_kit::test]
async fn cancelling_open_keeps_current_document_unchanged(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.dispatch_action(Box::new(Open), cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| None);
    cx.run_until_parked();

    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(workspace.active_document_path(), None);
        assert_eq!(workspace.document_text(id, cx), Some(String::new()));
        assert!(!workspace.document_is_dirty(id));
    });
}

#[gpui_kit::test]
async fn saves_dirty_document_to_selected_path(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("saved.txt");
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            let id = workspace.active_document_id();
            workspace.set_document_value(id, "Rueditor 内容", window, cx);
        });
        window.render_frame(cx);
        window.dispatch_action(Box::new(SaveAs), cx);
    })
    .unwrap();
    assert!(cx.did_prompt_for_new_path());
    let selected = destination.clone();
    cx.simulate_new_path_selection(move |_| Some(selected));
    cx.run_until_parked();

    assert_eq!(
        std::fs::read_to_string(&destination).unwrap(),
        "Rueditor 内容"
    );
    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(workspace.active_document_path(), Some(destination));
        assert!(!workspace.document_is_dirty(id));
    });
}
