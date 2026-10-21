# P1 Task 6：按文档隔离的正则查找面板与替换

## 实现

- `SearchSession` 是唯一的文档级搜索状态，继续保存在 `Document`；查询、替换内容、选项、面板状态和错误均属于该会话。两个 GPUI `InputState` 只由 `Workspace` 持有并在切换文档时从会话恢复。
- 增加打开/关闭面板、正则/大小写/全词切换、替换当前和全部替换 actions。工具栏及 GPUI Kit `Search` / `Replace` actions 打开紧凑工作区面板；Escape 关闭并恢复编辑器焦点。
- 普通模式仍使用 `EditorState::open_search`、`set_search_query`、前后匹配、当前替换和全部替换能力。组件原生搜索状态用于执行匹配，内置浮层随即关闭，由工作区紧凑面板负责呈现。
- 正则模式使用 Task 5 的 `find_matches` / `replace_matches`。前后查找从 selection/cursor 继续并循环；全部替换先计算完整新文本，成功后单次 `EditorState::replace_all`，保留一次撤销语义。
- 非法表达式和零匹配只更新会话错误，不触碰文本。headless 测试确认文本、revision 和 dirty 状态保持不变；捕获组 `$1` 替换确认仅增加一次 revision。

## TDD 记录

- RED：先添加工作区级会话隔离、正则全部替换和失败无副作用测试。focused 编译失败，指出缺少 `ToggleRegex` / `ReplaceAllMatches` actions 以及会话 setter/getter；确认测试针对尚未实现的 Task 6 工作流。
- 初次 action 测试未渲染 frame，action listener 尚未挂载；按测试框架实际生命周期，在切回文档后先 `render_frame`，并在离开 `Workspace::update` 后 dispatch action。随后 focused 行为测试通过。
- GREEN：两个 focused 测试分别通过；全量测试 48 passed、0 failed。

## 验证

- `cargo check --all-targets --offline`：通过。
- `cargo test --bin rueditor regex_search_sessions_are_isolated_per_document_and_replace_all_is_one_edit --offline`：通过。
- `cargo test --bin rueditor invalid_regex_keeps_text_revision_and_dirty_state_unchanged --offline`：通过，覆盖非法正则和零匹配。
- `cargo test --all-targets --offline`：48 passed、0 failed，包含既有普通搜索/替换及一次撤销回归。
- `cargo fmt --check`：通过。
- `git diff --check`：通过。

Cargo 对既有依赖 `block v0.1.6` 输出未来 Rust 兼容性提示；本次检查和测试均成功。真实窗口视觉、中文输入法及跨平台焦点行为未做人工验收。

未提交变更；原有 P0/P1 工作区内容保留。
