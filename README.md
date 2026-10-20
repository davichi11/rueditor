# Rueditor

Rueditor 是一个参考 Notepad++ 和 CotEditor 的轻量级跨平台文本编辑器，使用 Rust 2024 和 GPUI Kit 0.6.1 构建。项目聚焦于可靠的文本编辑、多文档工作区、语法高亮、代码折叠和常用查找替换流程。

## 当前状态

M0–M5 的核心功能已经实现：

| 里程碑 | 内容 |
| --- | --- |
| M0 | 项目标识统一为 `Rueditor` / `rueditor`，统一 Cargo 包、默认运行目标、窗口标题和应用 ID。 |
| M1 | 跨平台应用壳、窗口配置和平台主修饰键快捷键。macOS 使用 Command，Windows/Linux 使用 Ctrl。 |
| M2 | 新建、打开、拖入、保存、另存为；后台文件 I/O；UTF-8、UTF-8 BOM、LF 和 CRLF 处理；原子保存。 |
| M3 | 多标签文档工作区；每个文档独立维护文本、选区、滚动、撤销历史、路径、编码、换行格式和未保存状态。 |
| M4 | 常见文件扩展名自动识别；手动切换语言；语法高亮和代码折叠。 |
| M5 | 查找、替换、下一个匹配、上一个匹配和全部替换；全部替换支持一次撤销。 |
| P1 | 文档级编辑设置；跳转、复制、删除、移动、缩进和大小写转换等文本/行操作；普通文本与正则搜索替换；大小写、全词匹配和捕获组替换。 |

## 已实现功能

- 基本文本编辑、选区、撤销和重做。
- 多标签新建、切换和关闭。
- 打开多个文件；重复打开同一路径时定位已有标签。
- 将多个文件拖入窗口并逐个后台加载，单个文件失败不会阻塞其他文件。
- 关闭标签或退出应用时确认未保存修改；保存失败、取消保存或取消关闭不会丢失修改。
- 保存时保留 UTF-8 / UTF-8 BOM 和 LF / CRLF 格式。
- 拒绝非法 UTF-8、混合换行和单独 CR，并保留原文件不变。
- JSON 格式化；对部分常见非严格 JSON 内容进行自动修复后格式化。
- 普通文本格式化：清理行尾空白并统一文件末尾换行。
- 使用 CotEditor 风格的原生应用菜单栏，按“文件 / 编辑 / 格式 / 显示 / 文本 / 查找 / 窗口 / 帮助”组织操作，避免顶部工具栏拥挤。
- 文档标签栏保留当前文件和未保存状态；语言类型、编码和换行格式压缩显示在标题信息区和状态栏。

当前语言选择器提供：纯文本、Rust、Python、JavaScript、TypeScript、JSON、HTML、CSS、Markdown、TOML 和 YAML。未知类型回退为纯文本。

查找和替换支持普通文本与正则表达式模式，并提供大小写匹配、全词匹配、当前替换和全部替换。正则替换支持捕获组引用；无效正则表达式不会写回文档。普通文本搜索继续复用 GPUI Kit 的编辑器能力。

P1 编辑能力还包括文档级行号、自动换行、显示空白、智能缩进、自动闭合括号和 Tab 宽度设置，以及跳转到行、复制/删除/移动当前行、缩进/反缩进、大小写转换和清理行尾空白。设置和搜索会话均按文档独立保存，设置变化不会标记文档为未保存。

## 菜单栏与快捷键

菜单栏使用系统原生菜单承载完整操作，`secondary` 是 GPUI 的跨平台主修饰键：macOS 对应 Command，Windows/Linux 对应 Ctrl。

| 操作 | macOS | Windows/Linux |
| --- | --- | --- |
| 新建文档 | `⌘N` | `Ctrl+N` |
| 打开文件 | `⌘O` | `Ctrl+O` |
| 保存 | `⌘S` | `Ctrl+S` |
| 另存为 | `⇧⌘S` | `Ctrl+Shift+S` |
| 关闭标签 | `⌘W` | `Ctrl+W` |
| 退出应用 | `⌘Q` | `Ctrl+Q` |
| 查找 | `⌘F` | `Ctrl+F` |
| 替换 | `⇧⌘F` | `Ctrl+Shift+F` |
| 下一个匹配 | `⌘G` | `Ctrl+G` |
| 上一个匹配 | `⇧⌘G` | `Ctrl+Shift+G` |
| 跳转到行 | `⌘L` | `Ctrl+L` |
| 复制当前行 | `⌘D` | `Ctrl+D` |
| 删除当前行 | `⇧⌘K` | `Ctrl+Shift+K` |
| 移动当前行 | `⌥↑` / `⌥↓` | `Alt+Up` / `Alt+Down` |
| 缩进/反缩进 | `⌘[` / `⌘]` | `Ctrl+[` / `Ctrl+]` |

编辑区的输入、撤销和重做由 GPUI Kit 的 `Editor` 组件提供。

## 运行

需要 Rust stable、Cargo，以及 GPUI Kit 支持的桌面平台环境。

```sh
cargo run --bin rueditor
```

项目已经将 `rueditor` 配置为默认运行目标，因此下面的命令等价：

```sh
cargo run
```

离线环境可使用：

```sh
cargo run --offline
```

## 验证

```sh
cargo fmt --check
cargo check --all-targets --offline
cargo clippy --all-targets --offline -- -D warnings
cargo test --all-targets --offline
cargo build --release --offline
git diff --check
```

测试位于 [`src/text_editor/tests.rs`](src/text_editor/tests.rs)，覆盖文件读写、编码和换行、异步多文档流程、未保存状态、格式化、语言切换、折叠以及查找替换等行为。

## 代码结构

```text
src/bin/rueditor.rs         可执行入口
src/text_editor/mod.rs      编辑器初始化和主窗口生命周期
src/text_editor/shell.rs    平台快捷键和窗口启动配置
src/text_editor/document.rs 文档模型、路径、编码、换行和修订状态
src/text_editor/editor.rs   编辑器动作、语言识别和格式化工具
src/text_editor/file_io.rs  UTF-8/BOM、换行检测和原子文件保存
src/text_editor/workspace.rs 多文档工作区、标签、文件流程和主界面
src/text_editor/tests.rs    单元测试和 GPUI 集成测试
scripts/package-macos.sh    macOS App/DMG 打包脚本
```

工作区使用稳定的 `DocumentId` 管理文档。每个文档持有独立的 `EditorState`，异步文件操作完成后会根据文档 ID 和修订状态确认结果仍属于正确的文档。

## macOS 打包

打包脚本只在 macOS 上执行，默认生成未签名、未公证的 `dist/Rueditor.app`：

```sh
./scripts/package-macos.sh --offline
```

同时生成 DMG：

```sh
./scripts/package-macos.sh --offline --dmg
```

常用选项：

```text
--check             检查打包依赖、Info.plist、图标和已有 App，不构建
--offline           使用 Cargo 离线构建
--clean             清理本项目生成的 App 和 DMG
--dmg               在 App 校验后生成 Rueditor.dmg
--sign IDENTITY     使用 codesign 签名；传入 - 表示 ad-hoc 签名
```

脚本不会自动访问钥匙串、签名或公证。正式分发时应传入开发者证书 identity，并另行完成 notarization。

## 已知限制与后续工作

- 当前只处理 UTF-8 和 UTF-8 BOM，不自动转换其他字符编码。
- P0 已实现外部文件修改保护：保存前会检查文件指纹并拒绝覆盖外部修改；当前仍不支持自动重新载入和冲突合并。
- P1 已提供文档级编辑设置和自动换行；主题切换、字号调整等完整外观设置仍未产品化。
- 尚未针对超大文件实现分块加载或专门的性能优化。
- 尚未实现分栏编辑、文件夹级搜索、插件、LSP、内置终端、云同步、自动保存和 Unicode 正规化。
- Windows/Linux 原生窗口、中文输入法、原生拖放、最小窗口布局和真实视觉效果仍需在对应平台人工验收。

完整开发边界、阶段记录和验收顺序见[实现计划](docs/superpowers/plans/2026-09-14-text-editor.md)。
