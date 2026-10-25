#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
if [ "$(uname -s)" != Darwin ] || [ "$(uname -m)" != arm64 ]; then
  echo 'HarborVM requires Apple Silicon and macOS 14 or newer.' >&2
  exit 1
fi
cargo build --release
APP="$ROOT/dist/HarborVM.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/guest"
xcrun swiftc -swift-version 5 -O -target arm64-apple-macos14.0 \
  -framework AppKit -framework SwiftUI -framework Virtualization \
  platform/macos/*.swift -o "$APP/Contents/MacOS/HarborVM"
cp target/release/harborvm "$APP/Contents/MacOS/harborvm"
cp platform/macos/Info.plist "$APP/Contents/Info.plist"
cp guest/worker.py guest/setup.sh "$APP/Contents/Resources/guest/"
codesign --force --sign - "$APP/Contents/MacOS/harborvm"
codesign --force --sign - --entitlements platform/macos/HarborVM.entitlements "$APP"
codesign --verify --strict "$APP"
printf 'Built %s\n' "$APP"
