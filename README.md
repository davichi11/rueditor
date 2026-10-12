# Rueditor

Rueditor 是一个参考 Notepad++ 和 CotEditor 的轻量跨平台文本编辑器，使用 Rust 2024 和 GPUI Kit 0.6.1。当前 `rueditor` 分支专用于编辑器开发。

## 运行

```sh
cargo run
# 等价的显式入口
cargo run --bin rueditor
```

包名和默认可执行目标均为 `rueditor`。

## 当前状态

当前为单文档文件流程：使用 EditorState 持有内存文本、Editor 提供编辑界面，支持新建、打开、拖入、保存和另存为。多标签尚未实现；关闭窗口时会对未保存修改进行确认。

已下载 Tree-sitter 0.26.13 并启用 Rust、Python 和 JSON 高亮。三个样例标签独立保存内存文本、选区与撤销记录，底部行列随光标变化更新；折叠开关作用于所有样例。

已通过生产视图的 Unicode 输入、标签切换、撤销重做、折叠开关文本完整性测试，以及三种语言的解析和高亮颜色测试。测试中的 Unicode 输入不等同于系统中文输入法组合输入验收；实际点击折叠槽、嵌套折叠、折叠后编辑与搜索展开，以及原生窗口视觉检查仍待验证。

## 代码结构

```text
src/bin/rueditor.rs        极薄的可执行入口
src/text_editor/mod.rs    组件初始化和主窗口生命周期
src/text_editor/shell.rs  平台主修饰键和窗口启动配置
src/text_editor/editor.rs 单文档状态与编辑视图
src/text_editor/sample.rs 开发期样例文本
```

编辑实体跨帧保留，渲染只组合元素。文件操作、语言管理和搜索等模块将在对应阶段加入。已移除旧智能体应用说明。

M1 已完成应用壳的第一步：窗口配置集中管理，应用 ID 统一为 `local.rueditor.app`，应用级快捷键使用 GPUI 的 `secondary` 修饰键，在 macOS 上对应 Command，在 Windows/Linux 上对应 Ctrl。M2 已完成单文档文件流程和异步 I/O 自动化验证。Windows 原生窗口和输入法仍需在对应平台人工验收。

## 验证

```sh
cargo fmt --check
cargo check --all-targets --offline
cargo clippy --all-targets --offline -- -D warnings
cargo test --bin rueditor --offline
cargo build --bin rueditor --offline
```

交互与解析测试位于 `src/text_editor/tests.rs`，通过 `gpui-kit` 的 `test-support` 开发依赖执行。

完整范围与验收顺序见 [实现计划](docs/superpowers/plans/2026-09-14-text-editor.md)。

## macOS 打包

在 macOS 上执行以下命令生成 `dist/Rueditor.app`：

```sh
./scripts/package-macos.sh --offline
```

生成 App 并创建 DMG：

```sh
./scripts/package-macos.sh --offline --dmg
```

脚本默认不签名、不公证；本地验证可使用 `--sign -` 做 ad-hoc 签名，正式分发时传入开发者证书 identity。打包前可执行 `./scripts/package-macos.sh --check` 检查 Cargo、Info.plist 和图标资源。
