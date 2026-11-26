# Rueditor

基于 GPUI 构建的原生、轻量的跨平台文本编辑器：打开即编辑，不做 IDE。作为Notepad++ / CotEditor 的现代替代品。

![Rust](https://img.shields.io/badge/Rust-1.85%2B%20%7C%202024%20edition-000000?logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey)
![Version](https://img.shields.io/badge/version-1.0.0-blue)


## 这是什么

Rueditor 面向「打开一个文件、改几行、存盘、关掉」的场景：配置文件、日志、脚本、笔记，单文件从几百 KB 到几 MB。三个取舍决定了它的形状：

- **冷启动即可编辑。** 没有欢迎页、没有插件加载、没有工作区索引，窗口出现就能打字。
- **保留原编码和换行。** 可手动切换文件编码。目标编码无法无损表示正文时，保存会报错并保留原文件。
- **高亮和搜索做到够用，不追 IDE。** 37 种语言模式、代码折叠、正则替换、Markdown 预览，到此为止。

需要 LSP、调试器、Git 集成、内置终端的话，VS Code 或 Zed 是更合适的选择。

## 快速开始

### 下载安装（macOS）

从 [Releases](https://github.com/davichi11/rueditor/releases) 下载 `Rueditor.dmg`，拖入「应用程序」。

当前版本未签名、未公证，首次打开需要右键点图标 →「打开」，或执行：

```sh
xattr -dr com.apple.quarantine /Applications/Rueditor.app
```

### 从源码运行

需要 Rust 1.85 或更高（edition 2024）。

```sh
git clone git@github.com:davichi11/rueditor.git
cd rueditor
cargo run --release
```

`rueditor` 已是默认运行目标，`cargo run` 等价。离线环境加 `--offline`；想把文件直接带进来，追加路径即可：

```sh
cargo run --release -- ~/notes.md src/main.rs
```

## 功能

### 编辑

- 输入、选区、撤销与重做。
- 文档级设置：行号、自动换行、显示空白、智能缩进、自动闭合括号、Tab 宽度（2/4/8）。
- 行操作：跳转到行、复制 / 删除 / 移动当前行、缩进 / 反缩进、大小写转换、清理行尾空白。
- 设置和搜索会话按文档独立保存，改设置不会把文档标记成未保存。

### 文件与编码

- 新建、打开、拖入、保存、另存为；文件读写与哈希校验在后台完成，不阻塞界面。
- 多标签工作区，重复打开同一路径会定位到已有标签；批量拖入时单个文件失败不影响其他文件。
- 原子保存；保存前校验文件指纹，检测到外部修改时拒绝覆盖。
- 普通打开、拖放及系统打开都会自动识别文本编码，包括 UTF-8、UTF-16、GBK、GB18030、Big5、Shift_JIS、Windows-1252 等。也可以从状态栏手动指定编码。
- 点击状态栏的编码，可选择「以编码重新打开」或「以编码保存」。普通保存和另存为沿用文档编码与 LF / CRLF 换行。
- 解码失败、转码会丢失字符、混合换行或存在孤立 CR 时，操作会报错并保留原文件。
- 关闭标签或退出时确认未保存修改，保存失败或取消都不会丢内容。

### 语法高亮与折叠

内置 37 种语言模式（含纯文本回退），高亮由 tree-sitter 提供：

| 类别 | 支持 |
| --- | --- |
| 系统与脚本 | Shell / Bash、Makefile、CMake、Diff |
| 通用编程 | C、C++、Java、Go、C#、Kotlin、Swift、PHP、Ruby、Lua、Scala、Elixir、Zig、SQL |
| 前端 | JavaScript、JSX、TypeScript、TSX、HTML、CSS |
| 数据与配置 | JSON、TOML、YAML、XML、Properties、INI、Dotenv、Protocol Buffers、GraphQL |
| 文档 | Markdown |
| 其他 | Rust、Python、纯文本 |

- 检测顺序：显式扩展名 → 特殊文件名（Makefile、CMakeLists.txt、Gemfile、`.bashrc` 等）→ 首行 shebang（支持 `/usr/bin/env` 与 `env -S`）→ 纯文本。`.h` 归 C，`.hpp/.hh/.hxx` 与大写 `.C` 归 C++。
- 高亮区分函数、方法、参数、属性、JSX 标签、字符串转义与模板插值。
- 折叠：JS / TS / JSX / TSX 按真实块结构，Python 按函数、类与缩进分支，YAML 按层级；收起时完整隐藏块体并保留内层折叠状态。
- 主题跟随系统深浅色（Rueditor Light / Rueditor Dark），14px 界面字号、13px 等宽字号。

### 查找与替换

- 普通文本与正则两种模式，支持大小写匹配、全词匹配。
- 下一个 / 上一个匹配、替换当前、全部替换；正则替换支持捕获组引用，全部替换可一次撤销。
- 无效正则不会写回文档。

### 格式化

- JSON 格式化，并对部分常见的非严格 JSON 自动修复后格式化。
- 纯文本格式化：清理行尾空白、统一文件末尾换行。

### Markdown 预览

「显示 → Markdown 预览」或 `⇧⌘V` 打开右侧分栏，拖拽调整宽度，内容随正文实时刷新。按 CommonMark 与 GFM 解析，支持标题、列表、引用、表格、删除线和代码块，代码块复用编辑器的高亮主题。预览状态按文档保存，非 Markdown 文档打开时会提示不支持。

## 快捷键

菜单栏使用系统原生菜单，按「文件 / 编辑 / 格式 / 显示 / 文本 / 查找 / 窗口 / 帮助」组织。下表 macOS 列为 `secondary` 主修饰键，Windows/Linux 对应 Ctrl。

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
| 缩进 / 反缩进 | `⌘[` / `⌘]` | `Ctrl+[` / `Ctrl+]` |
| Markdown 预览 | `⇧⌘V` | `Ctrl+Shift+V` |

## 构建、测试与打包

```sh
cargo fmt --check
cargo check --all-targets --offline
cargo clippy --all-targets --offline -- -D warnings
cargo test --all-targets --offline
cargo build --release --offline
git diff --check
```

测试位于 [`src/text_editor/tests.rs`](src/text_editor/tests.rs)，覆盖文件读写、编码与换行、异步多文档流程、未保存状态、格式化、语言切换、折叠以及查找替换。

### macOS 打包

打包脚本只在 macOS 上执行，默认产出未签名、未公证的 `dist/Rueditor.app`：

```sh
./scripts/package-macos.sh --offline          # 只生成 App
./scripts/package-macos.sh --offline --dmg    # 同时生成 Rueditor.dmg
```

| 选项 | 作用 |
| --- | --- |
| `--check` | 检查依赖、Info.plist、图标和已有 App，不构建 |
| `--offline` | 使用 Cargo 离线构建 |
| `--clean` | 清理本项目生成的 App 和 DMG |
| `--dmg` | 在 App 校验后生成 `Rueditor.dmg` |
| `--sign IDENTITY` | 用 codesign 签名，传 `-` 表示 ad-hoc 签名 |

脚本不会自动访问钥匙串、签名或公证。正式分发需要传入开发者证书 identity，并另行完成 notarization。

## 代码结构

```text
src/bin/rueditor.rs            可执行入口
src/text_editor/mod.rs         初始化、窗口生命周期、启动参数与文件 URL
src/text_editor/shell.rs       平台快捷键、原生菜单和窗口配置
src/text_editor/appearance.rs  主题与配色（浅色 / 深色）
src/text_editor/workspace.rs   多文档工作区、标签、文件流程、Markdown 预览和主界面
src/text_editor/document.rs    文档模型、路径、编码、换行和修订状态
src/text_editor/editor.rs      动作定义、语言识别、搜索与格式化工具
src/text_editor/encoding.rs    编码与 BOM 处理
src/text_editor/file_io.rs     文本读取、换行检测、指纹与原子保存
src/text_editor/syntax.rs      高亮与折叠区间计算
src/text_editor/languages.rs   语法注册与查询修正
src/text_editor/languages/     自定义 tree-sitter 查询（.scm）
src/text_editor/chrome.rs      标签栏与状态栏的共享展示信息
src/text_editor/tests.rs       单元测试与 GPUI 集成测试
scripts/package-macos.sh       macOS App / DMG 打包脚本
```

工作区用稳定的 `DocumentId` 管理文档，每个文档持有独立的 `EditorState`。异步文件操作完成后，会根据文档 ID 和修订状态确认结果仍属于正确的文档。

## 已知限制

- 无 BOM 的短文本可能无法准确识别，可在状态栏选择「以编码重新打开」或「以编码打开」。
- 检测到外部修改时只拒绝保存，尚不支持自动重新载入与冲突合并。
- 外观只跟随系统深浅色，主题切换和字号调整尚未产品化。
- 未针对超大文件做分块加载或专门优化。
- 未实现分栏编辑、文件夹级搜索、插件、LSP、内置终端、云同步、自动保存和 Unicode 正规化。
- Windows/Linux 的原生窗口、中文输入法、原生拖放和最小窗口布局仍需实机验收。

完整阶段记录与后续计划见[路线图](docs/ROADMAP.md)。

## 许可

本项目采用 [MIT License](LICENSE)。
