//! 文件边界测试；后台打开使用平台选择器的测试模拟接口。

use super::super::document::{
    EditorSettings, Encoding, Newline, SearchMode, SearchOptions, SearchSession,
};
use super::super::editor::{
    DeleteLine, DuplicateLine, IndentSelection, JsonFormatResult, LineTransform,
    LowercaseSelection, MoveLineDown, MoveLineUp, Open, OutdentSelection, ReplaceAllMatches,
    SaveAs, TextTransform, ToggleRegex, ToggleSoftWrap, TrimTrailingWhitespace, UppercaseSelection,
    find_matches, format_json, language_for_path, parse_goto_position, remap_range_after_trimming,
    replace_matches, transform_current_line, transform_selection, trim_trailing_whitespace,
};
use super::super::file_io::{bytes_for_save, load, save_if_unchanged};
use super::super::shell::key_bindings;
use super::super::workspace::Workspace;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};
use std::io::ErrorKind;
use std::path::Path;

#[test]
fn parses_one_based_goto_line_and_optional_column() {
    assert_eq!(parse_goto_position("12:4", 20, 40).unwrap(), (11, 3));
    assert_eq!(parse_goto_position("7", 20, 40).unwrap(), (6, 0));
}

#[test]
fn rejects_invalid_or_out_of_range_goto_positions() {
    for input in ["", "0", "21", "1:0", "1:41", "1:x", "1:2:3"] {
        assert!(parse_goto_position(input, 20, 40).is_err(), "{input:?}");
    }
    assert!(parse_goto_position("1", 0, 40).is_err());
    assert!(parse_goto_position("1", 20, 0).is_err());
}

#[test]
fn transforms_unicode_selection_without_splitting_utf8() {
    assert_eq!(
        transform_selection("héllo 世界", 0..6, TextTransform::Uppercase).unwrap(),
        "HÉLLO 世界"
    );
    assert_eq!(
        transform_selection("HELLO 世界", 0..5, TextTransform::Lowercase).unwrap(),
        "hello 世界"
    );
    assert!(transform_selection("éx", 0..1, TextTransform::Uppercase).is_err());
    assert!(transform_selection("éx", 0..4, TextTransform::Uppercase).is_err());
    assert!(transform_selection("éx", 1..1, TextTransform::Uppercase).is_err());
}

#[test]
fn transforms_current_line_delete_and_move_while_preserving_final_newline() {
    let text = "甲\nmiddle\nlast\n";
    let cursor = text.find("middle").unwrap() + 2;
    assert_eq!(
        transform_current_line(text, cursor, LineTransform::Delete).unwrap(),
        "甲\nlast\n"
    );
    assert_eq!(
        transform_current_line(text, cursor, LineTransform::MoveUp).unwrap(),
        "middle\n甲\nlast\n"
    );
    assert_eq!(
        transform_current_line(text, cursor, LineTransform::MoveDown).unwrap(),
        "甲\nlast\nmiddle\n"
    );
    assert_eq!(
        transform_current_line(text, 0, LineTransform::MoveUp).unwrap(),
        text
    );
    assert_eq!(
        transform_current_line(text, text.len(), LineTransform::MoveDown).unwrap(),
        text
    );
}

#[test]
fn indents_and_outdents_one_current_line_without_touching_neighbors() {
    let text = "甲\n  value\nz";
    let cursor = text.find("value").unwrap() + 2;
    assert_eq!(
        transform_current_line(text, cursor, LineTransform::Indent).unwrap(),
        "甲\n      value\nz"
    );
    assert_eq!(
        transform_current_line("甲\n\tvalue\nz", cursor, LineTransform::Outdent).unwrap(),
        "甲\nvalue\nz"
    );
    assert_eq!(
        transform_current_line("甲\nvalue\nz", cursor, LineTransform::Outdent).unwrap(),
        "甲\nvalue\nz"
    );
}

#[test]
fn rejects_invalid_current_line_offsets_without_panicking() {
    assert!(transform_current_line("éx", 1, LineTransform::Delete).is_err());
    assert!(transform_current_line("éx", 4, LineTransform::Delete).is_err());
}

#[test]
fn trims_line_ending_whitespace_and_keeps_lf_text_cr_free() {
    assert_eq!(
        trim_trailing_whitespace("a  \n b\t\n尾　 \n"),
        "a\n b\n尾\n"
    );
    let normalized = trim_trailing_whitespace("a \r\nb\t\r");
    assert_eq!(normalized, "a\nb");
    assert!(!normalized.contains('\r'));
}

#[test]
fn remaps_selection_after_trailing_whitespace_is_removed() {
    assert_eq!(
        remap_range_after_trimming("a  \nvalue  \n", 4..11).unwrap(),
        2..7
    );
}

#[test]
fn editor_settings_use_lightweight_defaults_and_are_document_local() {
    let first = EditorSettings::default();
    let mut second = EditorSettings::default();

    assert!(first.show_line_numbers);
    assert!(!first.soft_wrap);
    assert!(!first.show_whitespaces);
    assert!(first.smart_indent);
    assert!(first.auto_close);
    assert_eq!(first.tab_size, 2);

    second.soft_wrap = true;
    assert_ne!(first.soft_wrap, second.soft_wrap);
}

#[test]
fn search_session_defaults_to_plain_case_insensitive_search() {
    let session = SearchSession::default();

    assert_eq!(session.options.mode, SearchMode::PlainText);
    assert!(!session.options.case_sensitive);
    assert!(!session.options.whole_word);
    assert!(session.query.is_empty());
}

#[test]
fn regex_search_and_replace_plain_mode_respects_case_and_returns_byte_ranges() {
    let sensitive = SearchOptions {
        mode: SearchMode::PlainText,
        case_sensitive: true,
        whole_word: false,
    };
    let insensitive = SearchOptions {
        case_sensitive: false,
        ..sensitive
    };

    assert_eq!(
        find_matches("Écho echo ECHO", "echo", sensitive).unwrap(),
        vec![6..10]
    );
    assert_eq!(
        find_matches("Écho echo ECHO", "echo", insensitive).unwrap(),
        vec![6..10, 11..15]
    );
}

#[test]
fn regex_search_and_replace_whole_word_uses_unicode_word_boundaries() {
    let options = SearchOptions {
        whole_word: true,
        ..SearchOptions::default()
    };
    let text = "猫猫 猫 _猫 猫_ e\u{301}x e\u{301} 猫";

    let cat_matches = find_matches(text, "猫", options).unwrap();
    assert_eq!(
        cat_matches
            .iter()
            .map(|range| &text[range.clone()])
            .collect::<Vec<_>>(),
        vec!["猫", "猫"]
    );
    assert_eq!(
        find_matches("e\u{301} e", "e", options).unwrap(),
        vec![4..5]
    );
}

#[test]
fn regex_search_and_replace_expands_capture_groups() {
    let options = SearchOptions {
        mode: SearchMode::Regex,
        case_sensitive: true,
        whole_word: false,
    };

    assert_eq!(
        find_matches("foo=12 bar=7", r"([a-z]+)=(\d+)", options).unwrap(),
        vec![0..6, 7..12]
    );
    let replaced = replace_matches("foo=12 bar=7", r"([a-z]+)=(\d+)", "$2:$1", options).unwrap();
    assert_eq!(replaced.text, "12:foo 7:bar");
    assert_eq!(replaced.count, 2);
}

#[test]
fn regex_search_and_replace_errors_preserve_input_and_report_details() {
    let plain = SearchOptions::default();
    let regex = SearchOptions {
        mode: SearchMode::Regex,
        ..plain
    };
    let original = "keep [";

    let empty_query = replace_matches(original, "", "changed", plain).unwrap_err();
    assert!(empty_query.to_string().contains("不能为空"));
    assert_eq!(original, "keep [");

    let invalid_regex = replace_matches(original, "(", "changed", regex).unwrap_err();
    assert!(invalid_regex.to_string().contains("("));
    assert_eq!(original, "keep [");

    let no_matches = replace_matches(original, "missing", "changed", plain).unwrap_err();
    assert!(no_matches.to_string().contains("匹配"));
    assert_eq!(original, "keep [");
    assert!(find_matches(original, "missing", plain).unwrap().is_empty());
}

#[test]
fn regex_search_and_replace_plain_replacement_keeps_dollar_text_literal() {
    let replaced = replace_matches("name", "name", "$1", SearchOptions::default()).unwrap();
    assert_eq!(replaced.text, "$1");
    assert_eq!(replaced.count, 1);
}

#[test]
fn detects_m4_languages_from_common_extensions() {
    for (path, language) in [
        ("main.rs", "rust"),
        ("script.py", "python"),
        ("app.ts", "typescript"),
        ("view.html", "html"),
        ("style.css", "css"),
        ("data.json", "json"),
        ("README.md", "markdown"),
        ("Cargo.toml", "toml"),
        ("config.yaml", "yaml"),
        ("component.tsx", "tsx"),
        ("main.go", "plaintext"),
        ("main.java", "plaintext"),
        ("main.cpp", "plaintext"),
        ("unknown.bin", "plaintext"),
    ] {
        assert_eq!(language_for_path(Path::new(path)), language, "{path}");
    }
}

#[test]
fn application_shortcuts_keep_quit_separate_from_close_tab() {
    let bindings = key_bindings();
    let action_for = |key: &str| {
        bindings
            .iter()
            .find(|binding| {
                binding.keystrokes().first().is_some_and(|keystroke| {
                    keystroke.inner().modifiers.secondary() && keystroke.inner().key == key
                })
            })
            .unwrap()
            .action()
            .name()
    };

    assert_eq!(action_for("w"), "text_editor::Close");
    assert_eq!(action_for("q"), "text_editor::Quit");
}

#[test]
fn external_file_changes_must_not_be_overwritten_by_save() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"original").unwrap();
    let loaded = load(file.path()).unwrap();

    std::fs::write(file.path(), b"external").unwrap();
    let result = save_if_unchanged(file.path(), b"memory", Some(&loaded.fingerprint));

    assert_eq!(result.unwrap_err().kind(), ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(file.path()).unwrap(), b"external");
}

#[test]
fn deleted_file_must_not_be_recreated_by_protected_save() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"original").unwrap();
    let loaded = load(file.path()).unwrap();
    std::fs::remove_file(file.path()).unwrap();

    let result = save_if_unchanged(file.path(), b"memory", Some(&loaded.fingerprint));

    assert_eq!(result.unwrap_err().kind(), ErrorKind::AlreadyExists);
    assert!(!file.path().exists());
}

#[test]
fn unchanged_file_can_be_saved_with_protection() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"original").unwrap();
    let loaded = load(file.path()).unwrap();

    save_if_unchanged(file.path(), b"memory", Some(&loaded.fingerprint)).unwrap();

    assert_eq!(std::fs::read(file.path()).unwrap(), b"memory");
}

#[test]
fn m5_shortcuts_include_find_and_replace() {
    let bindings = key_bindings();
    assert!(bindings.iter().any(|binding| {
        binding.keystrokes().first().is_some_and(|keystroke| {
            keystroke.inner().modifiers.secondary() && keystroke.inner().key == "f"
        })
    }));
    assert!(bindings.iter().any(|binding| {
        binding.keystrokes().first().is_some_and(|keystroke| {
            keystroke.inner().modifiers.secondary()
                && keystroke.inner().modifiers.shift
                && keystroke.inner().key == "f"
        })
    }));
    assert!(bindings.iter().any(|binding| {
        binding.keystrokes().first().is_some_and(|keystroke| {
            keystroke.inner().modifiers.secondary() && keystroke.inner().key == "g"
        })
    }));
    assert!(bindings.iter().any(|binding| {
        binding.keystrokes().first().is_some_and(|keystroke| {
            keystroke.inner().modifiers.secondary()
                && keystroke.inner().modifiers.shift
                && keystroke.inner().key == "g"
        })
    }));
}

#[test]
fn text_command_shortcuts_are_registered_with_expected_modifiers() {
    let bindings = key_bindings();
    for (key, shift, secondary, alt) in [
        ("l", false, true, false),
        ("d", false, true, false),
        ("k", true, true, false),
        ("up", false, false, true),
        ("down", false, false, true),
        ("[", false, true, false),
        ("]", false, true, false),
    ] {
        assert!(
            bindings.iter().any(|binding| {
                binding.keystrokes().first().is_some_and(|stroke| {
                    let modifiers = stroke.inner().modifiers;
                    stroke.inner().key == key
                        && modifiers.shift == shift
                        && modifiers.secondary() == secondary
                        && modifiers.alt == alt
                })
            }),
            "missing binding for {key}"
        );
    }
}

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
                workspace.activate_document_for_test(first, window, cx);
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
async fn changing_language_keeps_text_and_dirty_state(cx: &mut TestAppContext) {
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
            workspace.set_document_value(id, "fn main() {}", window, cx);
            let before = workspace.document_text(id, cx);
            workspace.set_document_language_for_test(id, "python", cx);
            assert_eq!(workspace.document_text(id, cx), before);
            assert!(workspace.document_is_dirty(id));
            assert_eq!(
                workspace.document_language_for_test(id),
                Some("python".to_string())
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn editor_search_replacement_updates_document_and_dirty_state(cx: &mut TestAppContext) {
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
            workspace.set_document_value(id, "alpha alpha", window, cx);
            workspace.mark_document_saved(id);
            workspace.replace_all_for_test(id, "alpha", "beta", window, cx);
        });
    })
    .unwrap();

    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(
            workspace.document_text(id, cx),
            Some("beta beta".to_string())
        );
        assert!(workspace.document_is_dirty(id));
    });
}

#[gpui_kit::test]
async fn editor_replace_all_is_one_undoable_operation(cx: &mut TestAppContext) {
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
            workspace.set_document_value(id, "alpha alpha", window, cx);
            workspace.replace_all_for_test(id, "alpha", "beta", window, cx);
            workspace.undo_document_for_test(id, window, cx);
        });
    })
    .unwrap();

    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(
            workspace.document_text(id, cx),
            Some("alpha alpha".to_string())
        );
    });
}

#[gpui_kit::test]
async fn regex_search_sessions_are_isolated_per_document_and_replace_all_is_one_edit(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();
    let mut first = None;
    let second = cx
        .update_window(handle.into(), |_, window, cx| {
            let second = view.update(cx, |workspace, cx| {
                let first_id = workspace.active_document_id();
                first = Some(first_id);
                workspace.set_document_value(first_id, "item=12 item=7", window, cx);
                workspace.set_search_session_for_test(
                    first_id,
                    SearchSession {
                        query: r"item=(\d+)".to_string(),
                        replacement: "$1".to_string(),
                        options: SearchOptions {
                            mode: SearchMode::Regex,
                            case_sensitive: true,
                            whole_word: false,
                        },
                        open: true,
                        replace_mode: true,
                        error: None,
                    },
                );
                workspace.new_tab(window, cx)
            });
            window.render_frame(cx);
            second
        })
        .unwrap();
    let first_id = first.unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).search_session_for_test(second),
            Some(SearchSession::default())
        )
    });

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.activate_document_for_test(first_id, window, cx);
        });
        window.render_frame(cx);
        window.dispatch_action(Box::new(ToggleRegex), cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx)
                .search_session_for_test(first_id)
                .unwrap()
                .options
                .mode,
            SearchMode::PlainText
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_action(Box::new(ToggleRegex), cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx)
                .search_session_for_test(first_id)
                .unwrap()
                .options
                .mode,
            SearchMode::Regex
        )
    });
    let revision = cx.update(|cx| view.read(cx).document_revision_for_test(first_id).unwrap());
    cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_action(Box::new(ReplaceAllMatches), cx);
    })
    .unwrap();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(
            workspace.document_text(first_id, cx),
            Some("12 7".to_string())
        );
        assert_eq!(
            workspace.document_revision_for_test(first_id),
            Some(revision + 1)
        );
        assert!(workspace.document_is_dirty(first_id));
        assert_eq!(
            workspace.search_session_for_test(first_id).unwrap().error,
            None
        );
    });
}

#[gpui_kit::test]
async fn invalid_regex_keeps_text_revision_and_dirty_state_unchanged(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();
    let id = cx.update(|cx| view.read(cx).active_document_id());
    let revision = cx
        .update_window(handle.into(), |_, window, cx| {
            let revision = view.update(cx, |workspace, cx| {
                let id = workspace.active_document_id();
                workspace.set_document_value(id, "keep this", window, cx);
                workspace.mark_document_saved(id);
                workspace.set_search_session_for_test(
                    id,
                    SearchSession {
                        query: "(".to_string(),
                        replacement: "changed".to_string(),
                        options: SearchOptions {
                            mode: SearchMode::Regex,
                            case_sensitive: false,
                            whole_word: false,
                        },
                        open: true,
                        replace_mode: true,
                        error: None,
                    },
                );
                workspace.document_revision_for_test(id).unwrap()
            });
            window.render_frame(cx);
            revision
        })
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_action(Box::new(ReplaceAllMatches), cx)
    })
    .unwrap();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(
            workspace.document_text(id, cx),
            Some("keep this".to_string())
        );
        assert_eq!(workspace.document_revision_for_test(id), Some(revision));
        assert!(!workspace.document_is_dirty(id));
        assert!(
            workspace
                .search_session_for_test(id)
                .unwrap()
                .error
                .is_some()
        );
    });
    cx.update(|cx| {
        view.update(cx, |workspace, _| {
            workspace.set_search_session_for_test(
                id,
                SearchSession {
                    query: "absent".to_string(),
                    replacement: "changed".to_string(),
                    options: SearchOptions {
                        mode: SearchMode::Regex,
                        case_sensitive: false,
                        whole_word: false,
                    },
                    open: true,
                    replace_mode: true,
                    error: None,
                },
            )
        });
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_action(Box::new(ReplaceAllMatches), cx)
    })
    .unwrap();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(
            workspace.document_text(id, cx),
            Some("keep this".to_string())
        );
        assert_eq!(workspace.document_revision_for_test(id), Some(revision));
        assert!(!workspace.document_is_dirty(id));
    });
}

#[gpui_kit::test]
async fn workspace_applies_text_commands_as_single_undoable_edits(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();
    let id = cx.update(|cx| view.read(cx).active_document_id());

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.set_document_value(id, "hello world\nsecond  \nthird", window, cx);
            workspace.mark_document_saved(id);
            workspace.set_document_selection_for_test(id, 0..5, cx);
        });
        window.render_frame(cx);
        window.dispatch_action(Box::new(UppercaseSelection), cx);
    })
    .unwrap();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(
            workspace.document_text(id, cx).as_deref(),
            Some("HELLO world\nsecond  \nthird")
        );
        assert_eq!(workspace.document_revision_for_test(id), Some(2));
        assert!(workspace.document_is_dirty(id));
    });

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.undo_document_for_test(id, window, cx)
        });
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).document_text(id, cx).as_deref(),
            Some("hello world\nsecond  \nthird")
        );
    });

    for (action, fixture, selection, expected) in [
        (
            "lower",
            "hello WORLD\nsecond  \nthird",
            6..11,
            "hello world\nsecond  \nthird",
        ),
        (
            "indent",
            "hello world\nsecond  \nthird",
            13..19,
            "hello world\n    second  \nthird",
        ),
        (
            "outdent",
            "hello world\n    second  \nthird",
            13..19,
            "hello world\nsecond  \nthird",
        ),
    ] {
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |workspace, cx| {
                workspace.set_document_value(id, fixture, window, cx);
                workspace.mark_document_saved(id);
                workspace.set_document_selection_for_test(id, selection, cx);
            });
            window.render_frame(cx);
            match action {
                "lower" => window.dispatch_action(Box::new(LowercaseSelection), cx),
                "indent" => window.dispatch_action(Box::new(IndentSelection), cx),
                _ => window.dispatch_action(Box::new(OutdentSelection), cx),
            }
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).document_text(id, cx).as_deref(),
                Some(expected)
            )
        });
    }

    for (action, expected) in [
        ("delete", "hello world\nthird"),
        ("up", "second  \nhello world\nthird"),
        ("down", "hello world\nthird\nsecond  "),
    ] {
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |workspace, cx| {
                workspace.set_document_value(id, "hello world\nsecond  \nthird", window, cx);
                workspace.mark_document_saved(id);
                workspace.set_document_cursor_for_test(id, 15, window, cx);
            });
            window.render_frame(cx);
            match action {
                "delete" => window.dispatch_action(Box::new(DeleteLine), cx),
                "up" => window.dispatch_action(Box::new(MoveLineUp), cx),
                _ => window.dispatch_action(Box::new(MoveLineDown), cx),
            }
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).document_text(id, cx).as_deref(),
                Some(expected)
            )
        });
    }

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.set_document_value(id, "hello world\nsecond  \nthird", window, cx);
            workspace.mark_document_saved(id);
        });
        window.render_frame(cx);
        window.dispatch_action(Box::new(TrimTrailingWhitespace), cx);
    })
    .unwrap();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(
            workspace.document_text(id, cx).as_deref(),
            Some("hello world\nsecond\nthird")
        );
        assert!(workspace.document_is_dirty(id));
    });

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.set_document_cursor_for_test(id, 0, window, cx);
            assert!(workspace.goto_line_for_test(id, "2:3", window, cx));
        });
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(view.read(cx).document_cursor_for_test(id, cx), Some((1, 2)));
    });
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.set_document_cursor_for_test(id, 0, window, cx);
            assert!(!workspace.goto_line_for_test(id, "99", window, cx));
        });
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(view.read(cx).document_cursor_for_test(id, cx), Some((0, 0)));
        assert_eq!(view.read(cx).message_for_test(), "行号超出范围");
    });

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.set_document_value(id, "one\ntwo", window, cx);
            workspace.set_document_cursor_for_test(id, 5, window, cx);
        });
        window.render_frame(cx);
        window.dispatch_action(Box::new(DuplicateLine), cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).document_text(id, cx).as_deref(),
            Some("one\ntwo\ntwo")
        );
    });
}

#[gpui_kit::test]
async fn toggling_folding_keeps_text_and_dirty_state(cx: &mut TestAppContext) {
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
            workspace.set_document_value(id, "fn main() {\n    println!(\"ok\");\n}\n", window, cx);
            let before = workspace.document_text(id, cx);
            workspace.set_document_folding_for_test(id, false, window, cx);
            assert_eq!(workspace.document_text(id, cx), before);
            assert!(workspace.document_is_dirty(id));
            assert_eq!(workspace.document_folding_for_test(id), Some(false));
            workspace.set_document_folding_for_test(id, true, window, cx);
            assert_eq!(workspace.document_text(id, cx), before);
            assert_eq!(workspace.document_folding_for_test(id), Some(true));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn document_settings_are_independent_and_do_not_mark_dirty(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();

    let (first, first_text, first_revision, first_was_dirty) = cx
        .update_window(handle.into(), |_, window, cx| {
            let baseline = view.update(cx, |workspace, cx| {
                let first = workspace.active_document_id();
                workspace.set_document_value(first, "first", window, cx);
                workspace.mark_document_saved(first);
                let first_text = workspace.document_text(first, cx).unwrap();
                let first_revision = workspace.document_revision_for_test(first).unwrap();
                let first_was_dirty = workspace.document_is_dirty(first);
                assert!(!first_was_dirty);
                assert!(
                    !workspace
                        .document_settings_for_test(first)
                        .unwrap()
                        .soft_wrap
                );
                (first, first_text, first_revision, first_was_dirty)
            });
            window.render_frame(cx);
            window.dispatch_action(Box::new(ToggleSoftWrap), cx);
            baseline
        })
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            view.read(cx)
                .document_settings_for_test(first)
                .unwrap()
                .soft_wrap
        );
    });

    let second = cx
        .update_window(handle.into(), |_, window, cx| {
            view.update(cx, |workspace, cx| {
                let second = workspace.new_tab(window, cx);
                workspace.set_document_line_numbers_for_test(second, false, window, cx);
                workspace.set_document_show_whitespaces_for_test(second, true, window, cx);
                workspace.set_document_smart_indent_for_test(second, false, window, cx);
                workspace.set_document_auto_close_for_test(second, false, window, cx);
                workspace.set_document_tab_size_for_test(second, 4, window, cx);
                second
            })
        })
        .unwrap();

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            assert!(!workspace.document_is_dirty(second));
            workspace.activate_document_for_test(first, window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();

    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.active_document_id(), first);
        assert_eq!(workspace.document_text(first, cx), Some(first_text));
        assert_eq!(
            workspace.document_revision_for_test(first),
            Some(first_revision)
        );
        assert_eq!(workspace.document_is_dirty(first), first_was_dirty);
        let first_settings = workspace.document_settings_for_test(first).unwrap();
        assert!(first_settings.soft_wrap);
        assert!(first_settings.show_line_numbers);
        let second_settings = workspace.document_settings_for_test(second).unwrap();
        assert!(!second_settings.show_line_numbers);
        assert!(second_settings.show_whitespaces);
        assert!(!second_settings.soft_wrap);
        assert_eq!(second_settings.tab_size, 4);
        assert_eq!(workspace.document_text(second, cx), Some(String::new()));
        assert_eq!(workspace.document_revision_for_test(second), Some(0));
        assert!(!workspace.document_is_dirty(second));
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
async fn saving_after_external_change_keeps_memory_and_disk_versions_separate(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "original").unwrap();
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
    let selected = path.clone();
    cx.simulate_path_prompt_response(move |_| Some(vec![selected]));
    cx.run_until_parked();

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            let id = workspace.active_document_id();
            workspace.set_document_value(id, "memory", window, cx);
        });
    })
    .unwrap();
    std::fs::write(&path, "external").unwrap();

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.dispatch_action(Box::new(super::super::editor::Save), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(workspace.document_text(id, cx), Some("memory".to_string()));
        assert!(workspace.document_is_dirty(id));
    });
    assert_eq!(std::fs::read_to_string(path).unwrap(), "external");
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
