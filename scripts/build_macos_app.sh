#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_NAME="BTCC Litedesk"
APP_BUNDLE_NAME="${APP_NAME}.app"
CRATE_DIR="examples/wallet"
ROOT_CARGO_TOML="$ROOT_DIR/Cargo.toml"
CRATE_CARGO_TOML="$ROOT_DIR/$CRATE_DIR/Cargo.toml"
THEMES_DIR="$CRATE_DIR/themes"
DIST_DIR="$ROOT_DIR/dist/macos"
APP_DIR="$DIST_DIR/$APP_BUNDLE_NAME"

ROOT_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT_CARGO_TOML" | head -n1)"
CRATE_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$CRATE_CARGO_TOML" | head -n1)"

if [[ -z "$ROOT_VERSION" ]]; then
  echo "无法从 $ROOT_CARGO_TOML 读取版本号" >&2
  exit 1
fi

if [[ -z "$CRATE_VERSION" ]]; then
  echo "无法从 $CRATE_CARGO_TOML 读取版本号" >&2
  exit 1
fi

if [[ "$ROOT_VERSION" != "$CRATE_VERSION" ]]; then
  echo "版本号不一致: root=$ROOT_VERSION, wallet=$CRATE_VERSION" >&2
  echo "请同步 Cargo.toml 和 examples/wallet/Cargo.toml 的 version" >&2
  exit 1
fi

VERSION="$ROOT_VERSION"
ARCH_NAME="$(uname -m)"
case "$ARCH_NAME" in
  arm64) TARGET_TRIPLE="aarch64-apple-darwin"; ARCH_LABEL="arm64" ;;
  x86_64) TARGET_TRIPLE="x86_64-apple-darwin"; ARCH_LABEL="x64" ;;
  *) echo "不支持的架构: $ARCH_NAME" >&2; exit 1 ;;
esac

BIN_PATH="$ROOT_DIR/target/$TARGET_TRIPLE/release/wallet"
DMG_NAME="BTCC-Litedesk-macos-${ARCH_LABEL}-${VERSION}.dmg"
DMG_PATH="$DIST_DIR/$DMG_NAME"

echo "正在构建 $TARGET_TRIPLE 版本..."

# ==================== 权限清理 ====================
echo "清理旧包权限..."
sudo chown -R "$USER" "$DIST_DIR" 2>/dev/null || true
chmod -R u+w "$DIST_DIR" 2>/dev/null || true
rm -rf "$APP_DIR"

# ==================== 构建二进制 ====================
rustup target add "$TARGET_TRIPLE" >/dev/null
cargo build --release -p wallet --target "$TARGET_TRIPLE"

if [[ ! -f "$BIN_PATH" ]]; then
  echo "❌ 二进制文件未找到: $BIN_PATH" >&2
  exit 1
fi

# ==================== 创建 .app 结构 ====================
echo "创建 App Bundle..."
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources/themes"

cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/$APP_NAME"
chmod +x "$APP_DIR/Contents/MacOS/$APP_NAME"
cp -R "$THEMES_DIR"/. "$APP_DIR/Contents/Resources/themes/"

# ==================== 图标处理 ====================
ICON_PATH="$APP_DIR/Contents/Resources/app.icns"
PNG_ICON_PATH="$CRATE_DIR/assets/app.png"

if [[ -f "$PNG_ICON_PATH" ]]; then
  echo "生成 icns 图标..."
  ICONSET_DIR="$DIST_DIR/app.iconset"
  rm -rf "$ICONSET_DIR"
  mkdir -p "$ICONSET_DIR"

  sips -z 16 16     "$PNG_ICON_PATH" --out "$ICONSET_DIR/icon_16x16.png" >/dev/null
  sips -z 32 32     "$PNG_ICON_PATH" --out "$ICONSET_DIR/icon_16x16@2x.png" >/dev/null
  sips -z 32 32     "$PNG_ICON_PATH" --out "$ICONSET_DIR/icon_32x32.png" >/dev/null
  sips -z 64 64     "$PNG_ICON_PATH" --out "$ICONSET_DIR/icon_32x32@2x.png" >/dev/null
  sips -z 128 128   "$PNG_ICON_PATH" --out "$ICONSET_DIR/icon_128x128.png" >/dev/null
  sips -z 256 256   "$PNG_ICON_PATH" --out "$ICONSET_DIR/icon_128x128@2x.png" >/dev/null
  sips -z 256 256   "$PNG_ICON_PATH" --out "$ICONSET_DIR/icon_256x256.png" >/dev/null
  sips -z 512 512   "$PNG_ICON_PATH" --out "$ICONSET_DIR/icon_256x256@2x.png" >/dev/null
  sips -z 512 512   "$PNG_ICON_PATH" --out "$ICONSET_DIR/icon_512x512.png" >/dev/null

  iconutil -c icns "$ICONSET_DIR" -o "$ICON_PATH" 2>/dev/null
fi

# ==================== Info.plist ====================
cat > "$APP_DIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
  <key>CFBundleDisplayName</key><string>${APP_NAME}</string>
  <key>CFBundleExecutable</key><string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key><string>com.btcc-litedesk.desktop</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>${APP_NAME}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleIconFile</key><string>app.icns</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF

# ==================== 创建 DMG ====================
echo "正在生成 DMG..."
rm -f "$DMG_PATH"
hdiutil create -volname "$APP_NAME" -srcfolder "$APP_DIR" -ov -format UDZO "$DMG_PATH" >/dev/null

SHA256_PATH="${DMG_PATH}.sha256"
HASH_VALUE="$(shasum -a 256 "$DMG_PATH" | awk '{print $1}')"
printf '%s  %s\n' "$HASH_VALUE" "$(basename "$DMG_PATH")" > "$SHA256_PATH"

echo "✅ 打包完成！"
echo "App Bundle: $APP_DIR"
echo "DMG 文件:   $DMG_PATH"
