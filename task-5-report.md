# P1 Task 5：纯搜索与正则替换引擎

## 实现

- 在 `editor.rs` 增加纯函数 `find_matches` 和 `replace_matches`，结果范围采用 UTF-8 byte offset。
- 普通文本通过 `regex::escape` 按字面量匹配；`RegexBuilder` 根据大小写选项配置匹配；正则替换使用 captures expansion，支持 `$1`、`$2` 等引用。
- whole-word 检查只看完整匹配两侧，并按 Unicode `\w` 边界过滤，覆盖字母、数字、组合标记和下划线。
- 空查询、无效正则和替换零匹配返回带具体说明的 `SearchError`；错误时不返回部分替换文本。
- `Cargo.toml` 声明 `regex = "1"`，`Cargo.lock` 已同步根包依赖。未改 workspace、shell 或 README。

## TDD 证据

先添加依赖与 5 个纯函数测试，再运行：

```text
cargo test --bin rueditor regex_search_and_replace --offline
```

RED：命令退出码 101，编译器提示测试导入的 `find_matches` 和 `replace_matches` 在 `text_editor::editor` 中不存在（E0432）。这确认测试针对的 API 尚未实现。

实现后首次运行有 4/5 通过；剩余断言把中文错误提示误检查为英文。将断言改为检查实际中文提示后，GREEN：同一 focused 命令通过，5 passed、0 failed。

## 验证

- `cargo fmt --check`：通过。
- `cargo test --bin rueditor regex_search_and_replace --offline`：5 passed、0 failed。
- `cargo test --all-targets --offline`：46 passed、0 failed。
- `git diff --check`：通过。
- Cargo 提示已有依赖 `block v0.1.6` 可能与未来 Rust 版本不兼容；当前构建和测试均通过。

工作树中原有 P0/P1 未提交内容保留；本任务未提交任何变更。仓库未发现独立 Task 5 计划文件，本实现依据本次任务规格和 README 中现有 M5 查找替换范围完成。
