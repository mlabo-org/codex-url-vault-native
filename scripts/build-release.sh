#!/bin/bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
app_name="Codex URL Vault.app"
app_root="$repo_root/dist/$app_name"
contents="$app_root/Contents"
icon_source="$repo_root/macos/Resources/AppIcon.png"
iconset="$repo_root/macos/.build/AppIcon.iconset"

cd "$repo_root"
MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release \
  -p url-vault-ffi \
  -p url-vault-mcp \
  -p url-vault-cli

mkdir -p "$repo_root/macos/Libraries"
cp "$repo_root/target/release/liburl_vault_ffi.a" \
  "$repo_root/macos/Libraries/liburl_vault_ffi.a"

cd "$repo_root/macos"
swift build -c release

mkdir -p "$contents/MacOS" "$contents/Resources"
rm -rf "$iconset"
mkdir -p "$iconset"
for icon_size in 16 32 128 256 512; do
  /usr/bin/sips -z "$icon_size" "$icon_size" "$icon_source" \
    --out "$iconset/icon_${icon_size}x${icon_size}.png" >/dev/null
  retina_size=$((icon_size * 2))
  /usr/bin/sips -z "$retina_size" "$retina_size" "$icon_source" \
    --out "$iconset/icon_${icon_size}x${icon_size}@2x.png" >/dev/null
done
/usr/bin/iconutil -c icns "$iconset" \
  -o "$contents/Resources/AppIcon.icns"
rm -rf "$iconset"

cp "$repo_root/macos/.build/release/CodexURLVault" \
  "$contents/MacOS/CodexURLVault"
cp "$repo_root/target/release/url-vault-mcp" \
  "$contents/Resources/url-vault-mcp"
cp "$repo_root/target/release/url-vault" \
  "$contents/Resources/url-vault"
cp "$repo_root/macos/Info.plist" "$contents/Info.plist"

chmod 755 \
  "$contents/MacOS/CodexURLVault" \
  "$contents/Resources/url-vault-mcp" \
  "$contents/Resources/url-vault"

/usr/bin/codesign --force --deep --sign - "$app_root"
printf '%s\n' "$app_root"
