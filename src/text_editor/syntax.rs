//! 编辑器语法适配：保留组件高亮，按语言结构生成折叠候选。
use gpui_kit::component::{
    highlighter::{LanguageRegistry, SyntaxHighlighter},
    input::{
        EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
        InputHighlighterFactory, Rope, RopeExt,
    },
};
use gpui_kit::{Context, HighlightStyle, SharedString, Task, Window};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
    time::Duration,
};

pub(super) fn factory() -> InputHighlighterFactory {
    Rc::new(|language| {
        LanguageRegistry::singleton()
            .language(language)?
            .language
            .as_ref()?;
        Some(Box::new(EditorSyntax {
            inner: Rc::new(RefCell::new(SyntaxHighlighter::new(language))),
            pending: None,
            worker_active: Rc::new(Cell::new(false)),
            request: Rc::new(RefCell::new(ParseRequest {
                text: Rope::new(),
                version: 0,
                needed: false,
            })),
            cached_folds: Rc::new(RefCell::new(Vec::new())),
        }))
    })
}

struct EditorSyntax {
    inner: Rc<RefCell<SyntaxHighlighter>>,
    pending: Option<Task<()>>,
    worker_active: Rc<Cell<bool>>,
    request: Rc<RefCell<ParseRequest>>,
    cached_folds: Rc<RefCell<Vec<FoldRange>>>,
}

struct ParseRequest {
    text: Rope,
    version: u64,
    needed: bool,
}

impl InputHighlighter for EditorSyntax {
    fn language(&self) -> SharedString {
        self.inner.borrow().language().clone()
    }

    fn update(
        &mut self,
        edit: Option<InputEdit>,
        text: &Rope,
        _folding: bool,
        window: &mut Window,
        cx: &mut Context<EditorState>,
    ) {
        let changed = self.inner.borrow().text() != text;
        let pending_same_text = self.request.borrow().needed && self.request.borrow().text == *text;
        let edit = edit.map(|edit| tree_sitter::InputEdit {
            start_byte: edit.start_byte,
            old_end_byte: edit.old_end_byte,
            new_end_byte: edit.new_end_byte,
            start_position: tree_sitter::Point::new(
                edit.start_position.row,
                edit.start_position.column,
            ),
            old_end_position: tree_sitter::Point::new(
                edit.old_end_position.row,
                edit.old_end_position.column,
            ),
            new_end_position: tree_sitter::Point::new(
                edit.new_end_position.row,
                edit.new_end_position.column,
            ),
        });
        let completed = if !changed && !pending_same_text {
            true
        } else if text.len() > 256 * 1024 {
            self.inner.borrow_mut().edit_tree(edit, text);
            false
        } else {
            self.inner
                .borrow_mut()
                .update(edit, text, Some(Duration::from_millis(2)))
        };
        // A timed-out parser already remembers the new text. A later layout
        // of that same text must not mistake its equality fast path for a
        // completed parse or cancel the pending worker.
        let completed = completed && !(pending_same_text && !changed);
        {
            let mut request = self.request.borrow_mut();
            if request.text != *text || request.needed == completed {
                request.version += 1;
            }
            request.text = text.clone();
            request.needed = !completed;
        }
        if completed {
            if changed {
                *self.cached_folds.borrow_mut() = fold_ranges(&self.inner.borrow());
            }
            // Base's incremental candidate merge retains enclosing ranges whose
            // header wasn't edited. Replace the complete semantic set after
            // that merge so moving a block boundary cannot leave stale folds.
            let inner = self.inner.clone();
            let snapshot = text.clone();
            let cached_folds = self.cached_folds.clone();
            cx.defer_in(window, move |state, _, cx| {
                if *state.text() == snapshot && state.language_name() == *inner.borrow().language()
                {
                    state.apply_highlighter_fold_candidates(cached_folds.borrow().clone(), cx);
                }
            });
        } else if !self.worker_active.replace(true) {
            // Keep one worker per adapter. Edits replace its latest request;
            // an in-flight parse finishes before it picks up the newest text.
            let inner = self.inner.clone();
            let language = self.language();
            let request = self.request.clone();
            let active = self.worker_active.clone();
            let cached_folds = self.cached_folds.clone();
            self.pending = Some(cx.spawn_in(window, async move |entity, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                loop {
                    let (snapshot, version) = {
                        let request = request.borrow();
                        if !request.needed {
                            break;
                        }
                        (request.text.clone(), request.version)
                    };
                    let parse_text = snapshot.clone();
                    let parse_language = language.clone();
                    let (parsed, folds) = cx
                        .background_executor()
                        .spawn(async move {
                            let mut parsed = SyntaxHighlighter::new(&parse_language);
                            parsed.update(None, &parse_text, None);
                            let folds = fold_ranges(&parsed);
                            (parsed, folds)
                        })
                        .await;
                    if request.borrow().version != version {
                        continue;
                    }
                    let _ = entity.update(cx, |state, cx| {
                        if *state.text() != snapshot || state.language_name() != *parsed.language()
                        {
                            return;
                        }
                        *inner.borrow_mut() = parsed;
                        *cached_folds.borrow_mut() = folds.clone();
                        state.apply_highlighter_fold_candidates(folds, cx);
                    });
                    request.borrow_mut().needed = false;
                    break;
                }
                active.set(false);
            }));
        }
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        self.inner.borrow().styles(range, resolver)
    }
    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        self.cached_folds.borrow().clone()
    }

    fn fold_ranges_for_edit(&self, range: Range<usize>, text: &Rope) -> Vec<FoldRange> {
        let start = text.offset_to_point(range.start.min(text.len())).row;
        let end = text.offset_to_point(range.end.min(text.len())).row;
        let folds = self.cached_folds.borrow();
        let first = folds.partition_point(|fold| fold.start_line < start);
        let last = folds.partition_point(|fold| fold.start_line <= end);
        folds[first..last].to_vec()
    }
}

pub(super) fn fold_ranges(highlighter: &SyntaxHighlighter) -> Vec<FoldRange> {
    let language = highlighter.language().as_ref();
    if language == "yaml" {
        return super::editor::yaml_fold_ranges(&highlighter.text().to_string())
            .into_iter()
            .map(|(start, end)| FoldRange::new(start, end))
            .collect();
    }
    let Some(tree) = highlighter.tree() else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        let kind = node.kind();
        let mut start = node.start_position().row;
        let mut end = node.end_position().row;
        if matches!(kind, "switch_case" | "switch_default") {
            end += 1;
        }
        let include = if language == "python" {
            if kind == "block" {
                start = node
                    .parent()
                    .map_or(start, |parent| parent.start_position().row);
                // The display map keeps the end row visible. Python has no
                // closing delimiter, so use the row after the last body line.
                end += 1;
                true
            } else {
                matches!(
                    kind,
                    "dictionary"
                        | "list"
                        | "set"
                        | "tuple"
                        | "argument_list"
                        | "parameters"
                        | "string"
                )
            }
        } else if matches!(language, "javascript" | "jsx" | "typescript" | "tsx") {
            matches!(
                kind,
                "switch_body"
                    | "switch_case"
                    | "switch_default"
                    | "interface_body"
                    | "enum_body"
                    | "object_type"
                    | "statement_block"
                    | "class_body"
                    | "object"
                    | "object_pattern"
                    | "array"
                    | "array_pattern"
                    | "arguments"
                    | "formal_parameters"
                    | "template_string"
                    | "comment"
                    | "jsx_element"
            )
        } else {
            // Keep the component's generic policy for other grammars.
            node.is_named() && node.parent().is_some()
        };
        if include && end.saturating_sub(start) >= 2 {
            ranges.push(FoldRange::new(start, end));
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                // A gutter has one handle per row. Choose the largest genuine
                // structural region on that row, never a wrapper expression.
                ranges.sort_by_key(|range| (range.start_line, std::cmp::Reverse(range.end_line)));
                ranges.dedup_by_key(|range| range.start_line);
                return ranges;
            }
        }
    }
}
