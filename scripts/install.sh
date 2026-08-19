#!/usr/bin/env sh
set -eu

VERSION=${DSH_DESKTOP_VERSION:-latest}
MIRROR=${DSH_DESKTOP_MIRROR:-}
REPO=${DSH_DESKTOP_REPO:-neko233-com/dsh-desktop}
INSTALL_DIR=${DSH_DESKTOP_INSTALL_DIR:-"$HOME/.local/share/dsh-desktop"}
BIN_DIR=${DSH_DESKTOP_BIN_DIR:-"$HOME/.local/bin"}
ASSET=dsh-desktop-macos-arm64.tar.gz
TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/dsh-desktop.XXXXXX")

cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT INT TERM

mirror_url() {
  url=$1
  if [ -z "$MIRROR" ]; then printf '%s\n' "$url"; return; fi
  case "$MIRROR" in
    *'{url}'*) printf '%s\n' "$MIRROR" | sed "s|{url}|$url|g" ;;
    *) printf '%s\n' "${MIRROR%/}/$url" ;;
  esac
}

if [ "$(uname -s)" != "Darwin" ]; then
  echo "仅支持 macOS。Windows 请使用 install.ps1。" >&2
  exit 1
fi
if [ "$(uname -m)" != "arm64" ]; then
  echo "仅支持 macOS arm64。" >&2
  exit 1
fi

if [ "$VERSION" = "latest" ]; then
  DIRECT="https://github.com/$REPO/releases/latest/download/$ASSET"
else
  DIRECT="https://github.com/$REPO/releases/download/$VERSION/$ASSET"
fi
ARCHIVE="$TMP_DIR/$ASSET"
downloaded=0
if [ -n "${DSH_DESKTOP_DOWNLOAD_URL:-}" ]; then
  URLS=${DSH_DESKTOP_DOWNLOAD_URL}
else
  URLS="$(mirror_url "$DIRECT")\n$DIRECT"
fi
printf '%b\n' "$URLS" | while IFS= read -r url; do
  [ -n "$url" ] || continue
  echo "下载 $url"
  if curl -fL --retry 3 --connect-timeout 15 "$url" -o "$ARCHIVE"; then
    downloaded=1
    break
  fi
done

if [ ! -s "$ARCHIVE" ]; then
  echo "没有可用发布包。请设置 DSH_DESKTOP_MIRROR，或下载源码后 cargo build --release。" >&2
  exit 1
fi

tar -xzf "$ARCHIVE" -C "$TMP_DIR"
APP=$(find "$TMP_DIR" -maxdepth 3 -name 'DSH Desktop.app' -type d | head -n 1)
if [ -z "$APP" ]; then
  echo "发布包缺少 DSH Desktop.app。" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR" "$BIN_DIR"
DEST="$INSTALL_DIR/DSH Desktop.app"
ditto "$APP" "$DEST"
if [ -n "${DSH_NPM_REGISTRY:-}" ]; then
  printf '%s' "$DSH_NPM_REGISTRY" > "$INSTALL_DIR/npm-registry"
fi
ln -sf "$DEST/Contents/MacOS/dsh-desktop" "$BIN_DIR/dsh-desktop"
echo "DSH Desktop 已安装：$DEST"
if ! command -v node >/dev/null 2>&1; then
  echo "警告：未检测到 Node.js 22+。首次启动 DSH 前请安装 Node.js，或设置 DSH_DESKTOP_DSH_BIN。" >&2
fi
echo "启动：open '$DEST'"
