#!/bin/sh
# Builds "Circuit Bench.app" for macOS into the dist folder.
# Makes a universal app (Apple chip + Intel) when both Rust targets are installed
# (rustup target add aarch64-apple-darwin x86_64-apple-darwin), otherwise one for this Mac.
set -e
cd "$(dirname "$0")"

APP="dist/Circuit Bench.app"
BIN_NAME="CircuitBench"
VERSION=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')

has_target() {
  command -v rustup >/dev/null 2>&1 && rustup target list --installed 2>/dev/null | grep -qx "$1"
}

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

if has_target aarch64-apple-darwin && has_target x86_64-apple-darwin; then
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  lipo -create -output "$APP/Contents/MacOS/$BIN_NAME" \
    target/aarch64-apple-darwin/release/circuit_bench \
    target/x86_64-apple-darwin/release/circuit_bench
  echo "Built a universal app (Apple chip + Intel)."
else
  cargo build --release
  cp target/release/circuit_bench "$APP/Contents/MacOS/$BIN_NAME"
  echo "Built an app for this Mac's processor."
fi
chmod +x "$APP/Contents/MacOS/$BIN_NAME"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Circuit Bench</string>
  <key>CFBundleDisplayName</key><string>Circuit Bench</string>
  <key>CFBundleIdentifier</key><string>io.github.circuitbench</string>
  <key>CFBundleExecutable</key><string>$BIN_NAME</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>10.13</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.education</string>
</dict>
</plist>
PLIST

echo "Done: $APP"
