#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAME="rueditor"
APP_NAME="Rueditor"
BUNDLE_ID="local.rueditor.app"
DIST_DIR="$ROOT_DIR/dist"
APP_DIR="$DIST_DIR/$APP_NAME.app"
DMG_PATH="$DIST_DIR/$APP_NAME.dmg"
PLIST_SOURCE="$ROOT_DIR/scripts/Info.plist"
ICON_SOURCE="$ROOT_DIR/assets/AppIcon.icns"

OFFLINE=0
CLEAN=0
CHECK_ONLY=0
MAKE_DMG=0
SIGN_IDENTITY=""

usage() {
    cat <<'EOF'
用法：scripts/package-macos.sh [选项]

选项：
  --check             只检查打包依赖、源文件和已有 App 结构，不构建
  --offline           使用 Cargo 离线构建
  --clean             删除本项目 dist/Rueditor.app 和 dist/Rueditor.dmg
  --dmg               在 App 通过校验后生成 Rueditor.dmg
  --sign IDENTITY     使用 codesign 签名；传入 - 表示 ad-hoc 签名
  -h, --help          显示帮助

默认行为：构建 release 版本并生成 dist/Rueditor.app。
脚本不会自动签名、公证或访问钥匙串。
EOF
}

die() {
    printf '错误：%s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || die "缺少命令：$1"
}

check_sources() {
    [[ "$(uname -s)" == "Darwin" ]] || die "此脚本只支持在 macOS 上执行"
    require_command cargo
    require_command plutil
    [[ -f "$ROOT_DIR/Cargo.toml" ]] || die "找不到 Cargo.toml：$ROOT_DIR/Cargo.toml"
    [[ -f "$ROOT_DIR/Cargo.lock" ]] || die "找不到 Cargo.lock，请先生成锁文件"
    [[ -x "$PLIST_SOURCE" || -f "$PLIST_SOURCE" ]] || die "找不到 Info.plist：$PLIST_SOURCE"
    [[ -f "$ICON_SOURCE" ]] || die "找不到 App 图标：$ICON_SOURCE"
    plutil -lint "$PLIST_SOURCE" >/dev/null || die "Info.plist 校验失败"

    local plist_executable plist_identifier plist_name
    plist_executable="$(plutil -extract CFBundleExecutable raw -o - "$PLIST_SOURCE")"
    plist_identifier="$(plutil -extract CFBundleIdentifier raw -o - "$PLIST_SOURCE")"
    plist_name="$(plutil -extract CFBundleName raw -o - "$PLIST_SOURCE")"
    [[ "$plist_executable" == "$BIN_NAME" ]] || die "CFBundleExecutable 必须为 $BIN_NAME"
    [[ "$plist_identifier" == "$BUNDLE_ID" ]] || die "CFBundleIdentifier 必须为 $BUNDLE_ID"
    [[ "$plist_name" == "$APP_NAME" ]] || die "CFBundleName 必须为 $APP_NAME"
}

package_version() {
    awk '
        $1 == "version" && $2 == "=" {
            gsub(/["\\r]/, "", $3)
            print $3
            exit
        }
    ' "$ROOT_DIR/Cargo.toml"
}

check_app() {
    [[ -d "$APP_DIR" ]] || die "找不到 App：$APP_DIR"
    [[ -f "$APP_DIR/Contents/Info.plist" ]] || die "App 缺少 Contents/Info.plist"
    [[ -x "$APP_DIR/Contents/MacOS/$BIN_NAME" ]] || die "App 缺少可执行文件或不可执行"
    [[ -f "$APP_DIR/Contents/Resources/AppIcon.icns" ]] || die "App 缺少 AppIcon.icns"
    plutil -lint "$APP_DIR/Contents/Info.plist" >/dev/null || die "打包后的 Info.plist 校验失败"

    local executable identifier name
    executable="$(plutil -extract CFBundleExecutable raw -o - "$APP_DIR/Contents/Info.plist")"
    identifier="$(plutil -extract CFBundleIdentifier raw -o - "$APP_DIR/Contents/Info.plist")"
    name="$(plutil -extract CFBundleName raw -o - "$APP_DIR/Contents/Info.plist")"
    [[ "$executable" == "$BIN_NAME" ]] || die "打包后的可执行文件名错误：$executable"
    [[ "$identifier" == "$BUNDLE_ID" ]] || die "打包后的 Bundle ID 错误：$identifier"
    [[ "$name" == "$APP_NAME" ]] || die "打包后的 App 名称错误：$name"

    printf 'App 结构校验通过：%s\n' "$APP_DIR"
}

build_app() {
    local version="$1"
    local cargo_args=(build --release --locked --bin "$BIN_NAME")
    if (( OFFLINE )); then
        cargo_args+=(--offline)
    fi

    printf '构建 Rueditor release binary…\n'
    (cd "$ROOT_DIR" && cargo "${cargo_args[@]}")

    local binary="$ROOT_DIR/target/release/$BIN_NAME"
    [[ -x "$binary" ]] || die "release binary 未生成：$binary"

    mkdir -p "$DIST_DIR"
    rm -rf "$APP_DIR"
    mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
    cp "$binary" "$APP_DIR/Contents/MacOS/$BIN_NAME"
    cp "$ICON_SOURCE" "$APP_DIR/Contents/Resources/AppIcon.icns"
    cp "$PLIST_SOURCE" "$APP_DIR/Contents/Info.plist"

    plutil -replace CFBundleShortVersionString -string "$version" \
        "$APP_DIR/Contents/Info.plist"
    plutil -replace CFBundleVersion -string "$version" \
        "$APP_DIR/Contents/Info.plist"
    chmod 755 "$APP_DIR/Contents/MacOS/$BIN_NAME"
    printf '已生成：%s\n' "$APP_DIR"
}

sign_app() {
    [[ -n "$SIGN_IDENTITY" ]] || return 0
    require_command codesign
    printf '签名 App（identity: %s）…\n' "$SIGN_IDENTITY"
    if [[ "$SIGN_IDENTITY" != "-" ]]; then
        codesign --force --deep --options runtime --timestamp \
            --sign "$SIGN_IDENTITY" "$APP_DIR"
    else
        codesign --force --deep --options runtime \
            --sign - "$APP_DIR"
    fi
    codesign --verify --deep --strict --verbose=2 "$APP_DIR"
}

make_dmg() {
    (( MAKE_DMG )) || return 0
    require_command hdiutil
    rm -f "$DMG_PATH"
    printf '生成 DMG：%s\n' "$DMG_PATH"
    hdiutil create \
        -volname "$APP_NAME" \
        -srcfolder "$APP_DIR" \
        -ov \
        -format UDZO \
        "$DMG_PATH" >/dev/null
    [[ -f "$DMG_PATH" ]] || die "DMG 未生成：$DMG_PATH"
}

while (($# > 0)); do
    case "$1" in
        --check)
            CHECK_ONLY=1
            ;;
        --offline)
            OFFLINE=1
            ;;
        --clean)
            CLEAN=1
            ;;
        --dmg)
            MAKE_DMG=1
            ;;
        --sign)
            (($# >= 2)) || die "--sign 需要一个 codesign identity"
            SIGN_IDENTITY="$2"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "未知选项：$1；使用 --help 查看用法"
            ;;
    esac
    shift
done

check_sources

if (( CLEAN )); then
    rm -rf "$APP_DIR" "$DMG_PATH"
    printf '已清理：%s 和 %s\n' "$APP_DIR" "$DMG_PATH"
fi

if (( CHECK_ONLY )); then
    if [[ -d "$APP_DIR" ]]; then
        check_app
    else
        printf '源文件校验通过；尚无已有 App：%s\n' "$APP_DIR"
    fi
    exit 0
fi

VERSION="$(package_version)"
[[ -n "$VERSION" ]] || die "无法从 Cargo.toml 读取版本"
build_app "$VERSION"
sign_app
check_app
make_dmg

printf '\n打包完成。\n'
printf 'App：%s\n' "$APP_DIR"
if (( MAKE_DMG )); then
    printf 'DMG：%s\n' "$DMG_PATH"
fi
