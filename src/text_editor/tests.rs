//! 文件边界测试；后台打开使用平台选择器的测试模拟接口。

use super::super::document::{
    EditorSettings, Encoding, Newline, SearchMode, SearchOptions, SearchSession,
};
use super::super::editor::{
    DeleteLine, DuplicateLine, IndentSelection, JsonFormatResult, LineTransform,
    LowercaseSelection, MoveLineDown, MoveLineUp, Open, OutdentSelection, ReplaceAllMatches,
    SaveAs, TextTransform, ToggleRegex, ToggleSoftWrap, TrimTrailingWhitespace, UppercaseSelection,
    find_matches, format_json, language_for_path, parse_goto_position, paths_from_open_urls,
    remap_range_after_trimming, replace_matches, supports_preview, transform_current_line,
    transform_selection, trim_trailing_whitespace, yaml_fold_ranges,
};
use super::super::file_io::{bytes_for_save, load, save_if_unchanged};
use super::super::shell::key_bindings;
use super::super::workspace::Workspace;
use gpui_kit::component::Root;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AppContext, ExternalPaths, FileDropEvent, InputEvent, TestAppContext, point, px, size,
};
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
        ("main.go", "go"),
        ("main.java", "java"),
        ("main.cpp", "cpp"),
        ("unknown.bin", "plaintext"),
    ] {
        assert_eq!(language_for_path(Path::new(path)), language, "{path}");
    }
}

#[test]
fn yaml_folding_tracks_nested_blocks_lists_and_blank_lines() {
    let text = "root:\n  child:\n    value: 1\n\n  items:\n    - first\n    - second\nnext: true\n";
    let ranges = yaml_fold_ranges(text);

    assert_eq!(ranges, vec![(0, 7), (1, 4), (4, 7)]);
}

#[test]
fn decodes_local_file_urls_and_ignores_other_schemes() {
    let paths = paths_from_open_urls([
        "file:///tmp/a%20b.yaml".to_string(),
        "https://example.com/file.txt".to_string(),
        "not a URL".to_string(),
    ]);

    assert_eq!(paths, vec![Path::new("/tmp/a b.yaml").to_path_buf()]);
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
async fn yaml_editor_installs_fold_candidates_for_each_indented_mapping(cx: &mut TestAppContext) {
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
            workspace.set_document_language_for_test(id, "yaml", cx);
            workspace.set_document_value(
                id,
                "root:\n  child:\n    value: 1\n\n  items:\n    - first\n    - second\nnext: true\n",
                window,
                cx,
            );
        });
        window.render_frame(cx);
    })
    .unwrap();

    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 0, cx),
            Some(true)
        );
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 1, cx),
            Some(true)
        );
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 4, cx),
            Some(true)
        );
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 2, cx),
            Some(false)
        );
    });
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
async fn dropping_a_file_on_the_window_opens_it_in_a_tab(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "dropped file").unwrap();
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
        window.dispatch_event(
            FileDropEvent::Entered {
                position: point(px(32.), px(32.)),
                paths: ExternalPaths([path.clone()].into_iter().collect()),
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(
            FileDropEvent::Submit {
                position: point(px(32.), px(32.)),
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();

    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(
            workspace.document_text(id, cx),
            Some("dropped file".to_string())
        );
        assert_eq!(workspace.active_document_path(), Some(path));
    });
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

#[test]
fn only_markdown_language_supports_preview() {
    assert!(supports_preview("markdown"));
    assert!(!supports_preview("rust"));
    assert!(!supports_preview("plaintext"));
    assert!(!supports_preview("json"));
}

#[test]
fn preview_markdown_grammar_covers_common_document_structures() {
    use markdown::mdast::Node;

    fn collect(node: &Node, kinds: &mut Vec<&'static str>) {
        let kind = match node {
            Node::Heading(_) => "heading",
            Node::List(_) => "list",
            Node::Blockquote(_) => "blockquote",
            Node::Code(_) => "code",
            Node::Table(_) => "table",
            Node::Strong(_) => "strong",
            Node::Paragraph(_) => "paragraph",
            _ => "other",
        };
        kinds.push(kind);
        if let Some(children) = node.children() {
            for child in children {
                collect(child, kinds);
            }
        }
    }

    let source = "# 标题\n\n段落 **加粗**\n\n- 列表项\n\n> 引用\n\n```rust\nlet x = 1;\n```\n\n| 列 | 列 |\n| --- | --- |\n| 值 | 值 |\n";
    let ast = markdown::to_mdast(source, &markdown::ParseOptions::gfm()).unwrap();
    let mut kinds = Vec::new();
    collect(&ast, &mut kinds);

    // 预览依赖 CommonMark 与 GFM：标题、列表、引用、代码块和表格都要能解析出来。
    for expected in ["heading", "list", "blockquote", "code", "table", "strong"] {
        assert!(
            kinds.contains(&expected),
            "预览应当支持 {expected} 语法，实际解析到：{kinds:?}"
        );
    }
}

#[gpui_kit::test]
async fn toggling_preview_on_markdown_document_enables_it(cx: &mut TestAppContext) {
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
            workspace.set_document_language_for_test(id, "markdown", cx);
            assert_eq!(workspace.document_preview_enabled_for_test(id), Some(false));

            workspace.toggle_preview_for_test(id, window, cx);
            assert_eq!(workspace.document_preview_enabled_for_test(id), Some(true));
            assert_eq!(workspace.message_for_test(), "已打开预览");

            workspace.toggle_preview_for_test(id, window, cx);
            assert_eq!(workspace.document_preview_enabled_for_test(id), Some(false));
            assert_eq!(workspace.message_for_test(), "已关闭预览");
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn toggling_preview_on_unsupported_language_is_rejected(cx: &mut TestAppContext) {
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
            workspace.set_document_language_for_test(id, "rust", cx);

            workspace.toggle_preview_for_test(id, window, cx);

            // 不支持的语言不打开面板，并给出明确提示。
            assert_eq!(workspace.document_preview_enabled_for_test(id), Some(false));
            assert_eq!(workspace.message_for_test(), "只有 Markdown 文档支持预览");
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn preview_setting_is_kept_per_document(cx: &mut TestAppContext) {
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
            let first = workspace.active_document_id();
            let second = workspace.new_tab(window, cx);
            workspace.set_document_language_for_test(first, "markdown", cx);
            workspace.set_document_language_for_test(second, "markdown", cx);

            workspace.toggle_preview_for_test(first, window, cx);

            // 两个标签各自记住自己的预览状态。
            assert_eq!(
                workspace.document_preview_enabled_for_test(first),
                Some(true)
            );
            assert_eq!(
                workspace.document_preview_enabled_for_test(second),
                Some(false)
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn toggling_preview_does_not_mark_document_as_dirty(cx: &mut TestAppContext) {
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
            workspace.set_document_language_for_test(id, "markdown", cx);
            workspace.set_document_value(id, "# 标题", window, cx);
            let revision = workspace.document_revision_for_test(id);

            workspace.toggle_preview_for_test(id, window, cx);

            // 预览是显示设置，不推进修订，也不影响未保存状态。
            assert_eq!(workspace.document_revision_for_test(id), revision);
            assert!(workspace.document_is_dirty(id));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn markdown_preview_panel_appears_only_after_toggling_it_on(cx: &mut TestAppContext) {
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
            workspace.set_document_language_for_test(id, "markdown", cx);
            workspace.set_document_value(id, "# 标题\n\n正文", window, cx);
        });
        window.render_frame(cx);

        // 还没打开预览时，画面里没有预览面板。
        assert!(
            window.try_find_by_label("Markdown 预览").is_none(),
            "预览面板不应在切换前出现"
        );

        let id = view.read(cx).active_document_id();
        view.update(cx, |workspace, cx| {
            workspace.toggle_preview_for_test(id, window, cx);
        });
        window.render_frame(cx);

        // 打开后预览面板确实被渲染出来。
        let panel = window.find_by_label("Markdown 预览");
        assert!(panel.visible());
        assert!(panel.bounds().size.width > px(0.));
    })
    .unwrap();
}

#[test]
fn detects_configuration_languages_and_dotenv_variants() {
    for (path, expected) in [
        ("pom.xml", "xml"),
        ("icon.svg", "xml"),
        ("App.config.XML", "xml"),
        ("messages.properties", "properties"),
        ("desktop.ini", "ini"),
        ("settings.cfg", "ini"),
        (".env", "dotenv"),
        (".env.local", "dotenv"),
        (".env.production.local", "dotenv"),
        ("service.env", "dotenv"),
        ("component.jsx", "jsx"),
        (".environment", "plaintext"),
        ("notes.env.txt", "plaintext"),
    ] {
        assert_eq!(language_for_path(Path::new(path)), expected, "{path}");
    }
}

/// Only the requested semantic capture gets a color, making missing or
/// shadowed captures observable through the same style runs the editor paints.
struct CaptureStyle(&'static str);
impl gpui_kit::base::input::HighlightStyleResolver for CaptureStyle {
    fn style(&self, name: &str) -> Option<gpui_kit::HighlightStyle> {
        (name == self.0).then(|| gpui_kit::HighlightStyle {
            color: Some(gpui_kit::rgb(0x123456).into()),
            ..Default::default()
        })
    }
}

fn assert_syntax_token(language: &str, source: &str, token: &str, capture: &'static str) {
    assert_syntax_with_resolver(language, source, token, capture, &CaptureStyle(capture));
}

fn assert_syntax_with_resolver(
    language: &str,
    source: &str,
    token: &str,
    capture: &str,
    resolver: &dyn gpui_kit::base::input::HighlightStyleResolver,
) {
    use gpui_kit::component::highlighter::SyntaxHighlighter;
    super::super::languages::init();
    let mut highlighter = SyntaxHighlighter::new(language);
    highlighter.update(None, &source.into(), None);
    let start = source.find(token).expect("fixture contains token");
    let end = start + token.len();
    let runs = highlighter.styles(&(start..end), resolver);
    let mut covered = start;
    for (range, style) in &runs {
        assert_eq!(
            range.start, covered,
            "{language}: missing coverage for {token:?}"
        );
        assert_eq!(
            style.color,
            Some(gpui_kit::rgb(0x123456).into()),
            "{language}: {token:?} should be {capture}, got {runs:?}"
        );
        covered = range.end;
    }
    assert_eq!(
        covered, end,
        "{language}: incomplete coverage for {token:?}"
    );
}

#[test]
fn javascript_highlights_jsx_tags_and_attributes() {
    let source = "const view = <Panel title=\"你好\">Hello</Panel>;";
    assert_syntax_token("javascript", source, "Panel", "tag");
    assert_syntax_token("javascript", source, "title", "attribute");
}

#[test]
fn javascript_highlights_escapes_and_template_expressions() {
    assert_syntax_token(
        "javascript",
        r#"const text = "hi\n";"#,
        r"\n",
        "string.escape",
    );
    assert_syntax_token(
        "javascript",
        "const msg = `Hi ${user.name}`;",
        "user",
        "variable",
    );
    assert_syntax_token(
        "javascript",
        "const msg = `Hi ${user.name}`;",
        "name",
        "property",
    );
}

#[test]
fn new_languages_produce_semantic_highlights() {
    for (language, source, token, capture) in [
        ("xml", "<root name=\"你好\"/>", "root", "tag"),
        (
            "properties",
            "server.port=8080\n",
            "server.port",
            "property",
        ),
        ("ini", "[server]\nport=8080\n", "server", "title"),
        (
            "dotenv",
            "export API_URL=\"https://example.test\"\n",
            "API_URL",
            "property",
        ),
    ] {
        assert_syntax_token(language, source, token, capture);
    }
}

#[test]
fn all_language_queries_compile_against_their_actual_grammar() {
    use gpui_kit::component::highlighter::LanguageRegistry;
    super::super::languages::init();
    for (name, _) in super::LANGUAGE_OPTIONS
        .iter()
        .filter(|(name, _)| *name != "plaintext")
    {
        let config = LanguageRegistry::singleton().language(name).expect(name);
        let grammar = config.language.as_ref().expect(name);
        let query = format!(
            "{}\n{}\n{}",
            config.injections, config.locals, config.highlights
        );
        tree_sitter::Query::new(grammar, &query).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

#[test]
fn ecmascript_highlights_specific_roles_before_generic_identifiers() {
    for language in ["javascript", "jsx", "typescript", "tsx"] {
        for (source, token, capture) in [
            (
                "function greet(person) { return person; }",
                "greet",
                "function",
            ),
            ("const greet = (person) => person;", "greet", "function"),
            ("service.fetchData();", "fetchData", "function.method"),
            ("const now = new Clock();", "Clock", "constructor"),
            ("const pattern = /ab+/gi;", "/ab+/gi", "string.regex"),
            ("const message = `你好 ${user.name}`;", "user", "variable"),
            ("const message = `你好 ${user.name}`;", "name", "property"),
            (r#"const message = "你好\n";"#, r"\n", "string.escape"),
        ] {
            assert_syntax_token(language, source, token, capture);
        }
    }
}

#[test]
fn typescript_highlights_typed_parameters_and_tsx_attributes() {
    assert_syntax_token(
        "typescript",
        "function greet(person: string) {}",
        "person",
        "variable.parameter",
    );
    assert_syntax_token(
        "tsx",
        "const view = <Panel title=\"Hello\"/>;",
        "Panel",
        "tag",
    );
    assert_syntax_token(
        "tsx",
        "const view = <Panel title=\"Hello\"/>;",
        "title",
        "attribute",
    );
}

#[test]
fn language_samples_keep_keywords_strings_and_comments_distinct() {
    for (language, source, token, capture) in [
        ("rust", "fn main() {}", "fn", "keyword"),
        ("python", "def main(): pass", "def", "keyword"),
        ("json", r#"{"name": "你好"}"#, r#""你好""#, "string"),
        ("html", "<div>Hello</div>", "div", "tag"),
        (
            "css",
            "/* comment */\nbody { color: red; }",
            "/* comment */",
            "comment",
        ),
        (
            "toml",
            "# comment\nname = \"hello\"\n",
            "# comment",
            "comment",
        ),
        ("yaml", "# comment\nname: hello\n", "# comment", "comment"),
        (
            "xml",
            "<!-- comment -->\n<root/>",
            "<!-- comment -->",
            "comment",
        ),
        ("xml", "<root>你好 &amp; 世界</root>", "&amp;", "constant"),
        (
            "properties",
            "# comment\nname=hello\n",
            "# comment",
            "comment",
        ),
        ("properties", "name=hello\\nworld\n", "\\n", "string.escape"),
        (
            "ini",
            "; comment\n[server]\nport=8080\n",
            "; comment",
            "comment",
        ),
        ("ini", "[server]\nport=8080\n", "8080", "string"),
        ("dotenv", "# comment\nAPI=hello\n", "# comment", "comment"),
        (
            "dotenv",
            "API=https://example.test\n",
            "https://example.test",
            "string",
        ),
        (
            "dotenv",
            "API=\"${HOST}/api\"\n",
            "HOST",
            "variable.special",
        ),
    ] {
        assert_syntax_token(language, source, token, capture);
    }
}

#[gpui_kit::test]
async fn folding_tracks_python_headers_and_js_blocks_instead_of_wrapper_nodes(
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
    for (language, source, expected) in [
        (
            "python",
            "def outer():\n    value = 1\n    if value:\n        print(value)\n    return value\n",
            vec![(0, true), (1, false), (2, true)],
        ),
        (
            "javascript",
            "const result = items\n    .map(item => {\n        return item;\n    })\n    .filter(Boolean);\n",
            vec![(0, false), (1, true)],
        ),
    ] {
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |workspace, cx| {
                let id = workspace.active_document_id();
                workspace.set_document_language_for_test(id, language, cx);
                workspace.set_document_value(id, source, window, cx);
            });
            window.render_frame(cx);
            let workspace = view.read(cx);
            for (line, expected) in expected {
                assert_eq!(
                    workspace.document_fold_candidate_for_test(
                        workspace.active_document_id(),
                        line,
                        cx
                    ),
                    Some(expected),
                    "{language} line {line}"
                );
            }
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn python_fold_ranges_hide_complete_bodies_and_preserve_nested_state(cx: &mut TestAppContext) {
    use gpui_kit::component::{highlighter::SyntaxHighlighter, input::DisplayMap};
    super::super::languages::init();
    let source = "class Service:\n    def run(self):\n        if self.ready:\n            work()\n        done()\n\nnext_task()\n";
    let rope = source.into();
    let mut highlighter = SyntaxHighlighter::new("python");
    highlighter.update(None, &rope, None);
    let folds = super::super::syntax::fold_ranges(&highlighter);
    assert_eq!(
        folds
            .iter()
            .map(|fold| (fold.start_line, fold.end_line))
            .collect::<Vec<_>>(),
        [(0, 5), (1, 5), (2, 4)]
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        let mut map = DisplayMap::new(gpui_kit::font("Menlo"), px(13.), None);
        map.set_text(&rope, cx);
        map.set_fold_candidates(folds);
        map.set_folded(2, true);
        assert!(map.is_buffer_line_hidden(3));
        assert!(!map.is_buffer_line_hidden(4));
        map.set_folded(0, true);
        assert!(map.is_buffer_line_hidden(4));
        assert!(!map.is_buffer_line_hidden(6));
        map.set_folded(0, false);
        assert!(!map.is_buffer_line_hidden(4));
        assert!(map.is_buffer_line_hidden(3));
    });
}

#[test]
fn js_fold_ranges_keep_each_nested_object_method_and_multiline_string() {
    use gpui_kit::component::highlighter::SyntaxHighlighter;
    super::super::languages::init();
    let source = "const config = {\n  nested: {\n    enabled: true,\n  },\n  run() {\n    return `hello {\nworld }`;\n  },\n};\nnextTask();\n";
    for language in ["javascript", "jsx", "typescript", "tsx"] {
        let mut highlighter = SyntaxHighlighter::new(language);
        highlighter.update(None, &source.into(), None);
        assert_eq!(
            super::super::syntax::fold_ranges(&highlighter)
                .iter()
                .map(|fold| (fold.start_line, fold.end_line))
                .collect::<Vec<_>>(),
            [(0, 8), (1, 3), (4, 7)],
            "{language}"
        );
    }
}

#[gpui_kit::test]
async fn new_language_menu_actions_preserve_text_revision_and_dirty_state(cx: &mut TestAppContext) {
    use super::super::editor::{
        SetBash, SetC, SetCMake, SetCSharp, SetCpp, SetDiff, SetDotenv, SetElixir, SetGo,
        SetGraphql, SetIni, SetJava, SetJsx, SetKotlin, SetLua, SetMake, SetPhp, SetProperties,
        SetProto, SetRuby, SetScala, SetSql, SetSwift, SetXml, SetZig,
    };
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
            workspace.set_document_value(workspace.active_document_id(), "保留文本\n", window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    let actions: Vec<(Box<dyn gpui_kit::Action>, &str)> = vec![
        (Box::new(SetXml), "xml"),
        (Box::new(SetProperties), "properties"),
        (Box::new(SetIni), "ini"),
        (Box::new(SetDotenv), "dotenv"),
        (Box::new(SetJsx), "jsx"),
        (Box::new(SetC), "c"),
        (Box::new(SetCpp), "cpp"),
        (Box::new(SetJava), "java"),
        (Box::new(SetGo), "go"),
        (Box::new(SetCSharp), "csharp"),
        (Box::new(SetKotlin), "kotlin"),
        (Box::new(SetSwift), "swift"),
        (Box::new(SetPhp), "php"),
        (Box::new(SetRuby), "ruby"),
        (Box::new(SetLua), "lua"),
        (Box::new(SetScala), "scala"),
        (Box::new(SetElixir), "elixir"),
        (Box::new(SetZig), "zig"),
        (Box::new(SetSql), "sql"),
        (Box::new(SetBash), "bash"),
        (Box::new(SetMake), "make"),
        (Box::new(SetCMake), "cmake"),
        (Box::new(SetProto), "proto"),
        (Box::new(SetGraphql), "graphql"),
        (Box::new(SetDiff), "diff"),
    ];
    let revision = cx.update(|cx| {
        let workspace = view.read(cx);
        workspace.document_revision_for_test(workspace.active_document_id())
    });
    for (action, language) in actions {
        cx.update_window(handle.into(), |_, window, cx| {
            window.dispatch_action(action, cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.update(|cx| {
            let workspace = view.read(cx);
            let id = workspace.active_document_id();
            assert_eq!(
                workspace.document_language_for_test(id).as_deref(),
                Some(language)
            );
            assert_eq!(
                workspace.document_text(id, cx).as_deref(),
                Some("保留文本\n")
            );
            assert_eq!(workspace.document_revision_for_test(id), revision);
            assert!(workspace.document_is_dirty(id));
        });
    }
}

#[test]
fn syntax_highlights_follow_edits_without_leaving_stale_string_ranges() {
    use gpui_kit::component::highlighter::SyntaxHighlighter;
    super::super::languages::init();
    for language in ["javascript", "python", "xml", "properties", "ini", "dotenv"] {
        let (before, after, token, role) = match language {
            "xml" => ("<root a=\"old\"/>", "<root a=\"世界\"/>", "世界", "string"),
            "properties" => ("name=old\n", "name=世界\n", "世界", "string"),
            "ini" => ("[app]\nname=old\n", "[app]\nname=世界\n", "世界", "string"),
            "dotenv" => ("NAME=old\n", "NAME=世界\n", "世界", "string"),
            _ => ("name = 'old'", "name = '世界'", "世界", "string"),
        };
        let mut highlighter = SyntaxHighlighter::new(language);
        highlighter.update(None, &before.into(), None);
        highlighter.update(None, &after.into(), None);
        let offset = after.find(token).unwrap();
        let runs = highlighter.styles(&(offset..offset + token.len()), &CaptureStyle(role));
        assert!(
            runs.iter().all(|(_, style)| style.color.is_some()),
            "{language} {runs:?}"
        );
    }
}

#[test]
fn javascript_switch_and_typescript_type_bodies_remain_foldable() {
    use gpui_kit::component::highlighter::SyntaxHighlighter;
    super::super::languages::init();
    for (language, source) in [
        (
            "javascript",
            "switch (value) {\n  case 1:\n    work();\n    break;\n  default:\n    done();\n}\n",
        ),
        (
            "typescript",
            "interface Config {\n  name: string;\n  port: number;\n}\n",
        ),
        ("typescript", "enum Mode {\n  Read,\n  Write,\n}\n"),
    ] {
        let mut highlighter = SyntaxHighlighter::new(language);
        highlighter.update(None, &source.into(), None);
        assert!(
            super::super::syntax::fold_ranges(&highlighter)
                .iter()
                .any(|fold| fold.start_line == 0),
            "{source}"
        );
    }
}

#[gpui_kit::test]
async fn background_folding_uses_latest_large_document_and_survives_language_switch(
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
    let mut large = "# padding\n".repeat(27_000);
    large.push_str("def outer():\n    value = 1\n    if value:\n        work()\n    done()\n");
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            let id = workspace.active_document_id();
            workspace.set_document_language_for_test(id, "python", cx);
            workspace.set_document_value(id, &large, window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(200));
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 27_000, cx),
            Some(true)
        );
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 27_001, cx),
            Some(false)
        );
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 27_002, cx),
            Some(true)
        );
    });
    // Queue another large parse, then replace it and switch language before
    // its debounce expires. The old snapshot must never restore stale folds.
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| {
            let id = workspace.active_document_id();
            workspace.set_document_value(id, &(large + "# edited\n"), window, cx);
        });
        window.render_frame(cx);
        view.update(cx, |workspace, cx| {
            let id = workspace.active_document_id();
            workspace.set_document_language_for_test(id, "javascript", cx);
            workspace.set_document_value(
                id,
                "const result = items\n  .map(item => {\n    return item;\n  });\n",
                window,
                cx,
            );
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(200));
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        let id = workspace.active_document_id();
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 0, cx),
            Some(false)
        );
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 1, cx),
            Some(true)
        );
        assert_eq!(
            workspace.document_fold_candidate_for_test(id, 27_000, cx),
            Some(false)
        );
    });
}

#[gpui_kit::test]
async fn reenabling_folding_refreshes_edits_made_while_disabled(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();
    for padding in [0, 27_000] {
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |workspace, cx| {
                let id = workspace.active_document_id();
                workspace.set_document_language_for_test(id, "python", cx);
                workspace.set_document_folding_for_test(id, false, window, cx);
                let text = "# padding\n".repeat(padding)
                    + "def outer():\n    if ready:\n        work()\n    done()\n";
                workspace.set_document_value(id, &text, window, cx);
            });
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |workspace, cx| {
                let id = workspace.active_document_id();
                workspace.set_document_folding_for_test(id, true, window, cx);
            });
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let workspace = view.read(cx);
            let id = workspace.active_document_id();
            assert_eq!(
                workspace.document_fold_candidate_for_test(id, padding, cx),
                Some(true)
            );
            assert_eq!(
                workspace.document_fold_candidate_for_test(id, padding + 1, cx),
                Some(true)
            );
            assert_eq!(
                workspace.document_fold_candidate_for_test(id, padding + 2, cx),
                Some(false)
            );
        });
    }
}

#[test]
fn mainstream_language_paths_detect_sources_headers_and_build_files() {
    for (path, expected) in [
        ("main.c", "c"),
        ("api.h", "c"),
        ("main.C", "cpp"),
        ("main.cpp", "cpp"),
        ("api.hpp", "cpp"),
        ("api.hxx", "cpp"),
        ("Main.java", "java"),
        ("main.go", "go"),
        ("Program.cs", "csharp"),
        ("main.kt", "kotlin"),
        ("build.gradle.kts", "kotlin"),
        ("main.swift", "swift"),
        ("index.php", "php"),
        ("view.phtml", "php"),
        ("app.rb", "ruby"),
        ("Gemfile", "ruby"),
        ("Rakefile", "ruby"),
        ("main.lua", "lua"),
        ("main.scala", "scala"),
        ("build.sbt", "scala"),
        ("main.ex", "elixir"),
        ("test.exs", "elixir"),
        ("build.zig", "zig"),
        ("schema.sql", "sql"),
        ("setup.sh", "bash"),
        (".bashrc", "bash"),
        (".zshrc", "bash"),
        ("configure", "bash"),
        ("Makefile", "make"),
        ("GNUmakefile", "make"),
        ("rules.mk", "make"),
        ("CMakeLists.txt", "cmake"),
        ("toolchain.cmake", "cmake"),
        ("service.proto", "proto"),
        ("schema.graphql", "graphql"),
        ("query.gql", "graphql"),
        ("change.patch", "diff"),
        ("change.diff", "diff"),
        ("stub.pyi", "python"),
        ("script.pyw", "python"),
        ("module.mts", "typescript"),
        ("module.cts", "typescript"),
        ("config.jsonc", "json"),
        ("README.mdx", "markdown"),
        ("unknown.xyz", "plaintext"),
        ("Makefile.txt", "plaintext"),
    ] {
        assert_eq!(language_for_path(Path::new(path)), expected, "{path}");
    }
}

#[test]
fn mainstream_languages_highlight_comments_strings_and_keywords() {
    for (language, source, comment, string, keyword) in [
        (
            "c",
            "// note\nint main() { return puts(\"hello\"); }",
            "// note",
            "\"hello\"",
            "return",
        ),
        (
            "cpp",
            "// note\nint main() { return puts(\"hello\"); }",
            "// note",
            "\"hello\"",
            "return",
        ),
        (
            "java",
            "// note\nclass Main { String run() { return \"hello\"; } }",
            "// note",
            "\"hello\"",
            "return",
        ),
        (
            "go",
            "// note\npackage main\nfunc run() string { return \"hello\" }",
            "// note",
            "\"hello\"",
            "return",
        ),
        (
            "csharp",
            "// note\nclass Main { string Run() { return \"hello\"; } }",
            "// note",
            "\"hello\"",
            "return",
        ),
        (
            "kotlin",
            "// note\nfun run(): String { return \"hello\" }",
            "// note",
            "hello",
            "return",
        ),
        (
            "swift",
            "// note\nfunc run() -> String { return \"hello\" }",
            "// note",
            "hello",
            "return",
        ),
        (
            "php",
            "<?php // note\nfunction run() { return 'hello'; }",
            "// note",
            "'hello'",
            "return",
        ),
        (
            "ruby",
            "# note\ndef run\n  return 'hello'\nend",
            "# note",
            "'hello'",
            "return",
        ),
        (
            "lua",
            "-- note\nfunction run() return 'hello' end",
            "-- note",
            "'hello'",
            "return",
        ),
        (
            "scala",
            "// note\ndef run(): String = { return \"hello\" }",
            "// note",
            "\"hello\"",
            "return",
        ),
        (
            "elixir",
            "# note\nif true, do: \"hello\"",
            "# note",
            "hello",
            "if",
        ),
        (
            "zig",
            "// note\nfn run() []const u8 { return \"hello\"; }",
            "// note",
            "\"hello\"",
            "return",
        ),
        (
            "sql",
            "-- note\nSELECT 'hello' FROM users;",
            "-- note",
            "'hello'",
            "SELECT",
        ),
        (
            "bash",
            "# note\nif true; then echo \"hello\"; fi",
            "# note",
            "\"hello\"",
            "if",
        ),
        (
            "cmake",
            "# note\nif(TRUE)\nmessage(\"hello\")\nendif()",
            "# note",
            "hello",
            "if",
        ),
        (
            "proto",
            "// note\nsyntax = \"proto3\";\nmessage Sample { string name = 1; }",
            "// note",
            "\"proto3\"",
            "message",
        ),
        (
            "graphql",
            "# note\nquery Sample { user(name: \"hello\") { id } }",
            "# note",
            "\"hello\"",
            "query",
        ),
    ] {
        assert_syntax_with_resolver(
            language,
            source,
            comment,
            "comment",
            &CaptureFamily("comment"),
        );
        assert_syntax_with_resolver(language, source, string, "string", &CaptureFamily("string"));
        assert_syntax_with_resolver(
            language,
            source,
            keyword,
            "keyword",
            &CaptureFamily("keyword"),
        );
    }
    assert_syntax_token("make", "# note\nall:\n\techo hello\n", "# note", "comment");
    assert_syntax_token(
        "diff",
        "--- a/a\n+++ b/a\n@@ -1 +1 @@\n-old\n+new\n",
        "+new",
        "string",
    );
}

#[gpui_kit::test]
async fn opening_extensionless_scripts_uses_shebang_without_overriding_known_types(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let dir = tempfile::tempdir().unwrap();
    let mut view = None;
    let handle = cx.open_window(size(px(720.), px(480.)), |window, cx| {
        let workspace = cx.new(|cx| Workspace::new(window, cx));
        view = Some(workspace.clone());
        Root::new(workspace, window, cx)
    });
    let view = view.unwrap();
    for (name, source, expected) in [
        (
            "python-tool",
            "#!/usr/bin/env python3\nprint('hello')\n",
            "python",
        ),
        (
            "node-tool",
            "#!/usr/bin/env -S node --no-warnings\nconsole.log('hello');\n",
            "javascript",
        ),
        ("ruby-tool", "#!/usr/bin/ruby\nputs 'hello'\n", "ruby"),
        ("shell-tool", "#!/bin/bash -eu\necho hello\n", "bash"),
        (
            "lua-tool",
            "\u{feff}#!/usr/bin/env lua5.4\nprint('hello')\n",
            "lua",
        ),
        (
            "unsupported-tool",
            "#!/usr/bin/env fish\necho hello\n",
            "plaintext",
        ),
        ("not-a-shebang", "# python\nprint('hello')\n", "plaintext"),
        (
            "notes.txt",
            "#!/usr/bin/python3\nprint('hello')\n",
            "plaintext",
        ),
        (
            "actual.js",
            "#!/usr/bin/python3\nprint('hello')\n",
            "javascript",
        ),
        (
            "env-trap",
            "#!/usr/bin/env CUSTOM=python3 unknown\n",
            "plaintext",
        ),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, source).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |workspace, cx| {
                workspace.open_paths(vec![path], window, cx)
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let workspace = view.read(cx);
            let id = workspace.active_document_id();
            assert_eq!(
                workspace.document_language_for_test(id).as_deref(),
                Some(expected),
                "{name}"
            );
        });
    }
}

// Match the theme's documented dotted-name fallback, while the specific-role
// regressions above continue to use the exact CaptureStyle resolver.
struct CaptureFamily(&'static str);
impl gpui_kit::base::input::HighlightStyleResolver for CaptureFamily {
    fn style(&self, name: &str) -> Option<gpui_kit::HighlightStyle> {
        (name == self.0
            || name
                .strip_prefix(self.0)
                .is_some_and(|suffix| suffix.starts_with('.')))
        .then(|| gpui_kit::HighlightStyle {
            color: Some(gpui_kit::rgb(0x123456).into()),
            ..Default::default()
        })
    }
}

#[test]
fn mainstream_language_specific_roles_are_not_hidden_by_generic_identifiers() {
    for (language, source, token, family) in [
        (
            "c",
            "int greet(int value) { return value; }",
            "greet",
            "function",
        ),
        (
            "cpp",
            "class Sample {}; int greet(int value) { return value; }",
            "Sample",
            "type",
        ),
        (
            "java",
            "class Sample { String greet() { return \"hello\"; } }",
            "Sample",
            "type",
        ),
        (
            "java",
            "class Sample { String greet() { return \"hello\"; } }",
            "greet",
            "function",
        ),
        (
            "go",
            "package main\nfunc greet() string { return \"hello\" }",
            "greet",
            "function",
        ),
        (
            "csharp",
            "class Sample { string Greet() { return \"hello\"; } }",
            "Sample",
            "type",
        ),
        (
            "csharp",
            "class Sample { string Greet() { return \"hello\"; } }",
            "Greet",
            "function",
        ),
        (
            "swift",
            "func greet(person: String) -> String { return person }",
            "greet",
            "function",
        ),
        (
            "swift",
            "func greet(person: String) -> String { return person }",
            "person",
            "variable.parameter",
        ),
        (
            "kotlin",
            "fun greet(person: String): String { return person }",
            "greet",
            "function",
        ),
        (
            "ruby",
            "def greet(person)\n  result = person\n  result\nend",
            "greet",
            "function",
        ),
        (
            "ruby",
            "def greet(person)\n  result = person\n  result\nend",
            "person",
            "variable.parameter",
        ),
        (
            "ruby",
            "def greet(person)\n  result = person\n  result\nend",
            "result",
            "variable",
        ),
        (
            "lua",
            "local function greet(person) return person end",
            "greet",
            "function",
        ),
        (
            "scala",
            "def greet(person: String): String = person",
            "person",
            "variable.parameter",
        ),
        (
            "proto",
            "syntax = \"proto3\"; message Sample { string name = 1; }",
            "Sample",
            "type",
        ),
        (
            "graphql",
            "query Sample { user(name: \"hello\") { id } }",
            "user",
            "property",
        ),
        ("cmake", "message(\"hello\")", "message", "function"),
    ] {
        assert_syntax_with_resolver(language, source, token, family, &CaptureFamily(family));
    }
}

#[test]
fn lua_table_methods_use_function_highlighting() {
    for source in [
        "module.greet()",
        "function module.greet(person) return person end",
    ] {
        assert_syntax_with_resolver(
            "lua",
            source,
            "greet",
            "function",
            &CaptureFamily("function"),
        );
    }
}
