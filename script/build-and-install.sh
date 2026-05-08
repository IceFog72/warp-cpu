#!/usr/bin/env bash
#
# Build and install Warp to the system.
#
# Usage:
#   ./script/build-and-install.sh [--channel oss|dev|local]
#
# Environment variables:
#   WARP_FORCE_SOFTWARE=1   Force CPU software rendering (llvmpipe)
#   WARP_SOFTWARE_FPS=30    Cap FPS for software rendering (default 30)
#

set -e

WORKSPACE_ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$WORKSPACE_ROOT_DIR"

CHANNEL="${1:-oss}"
CARGO_PROFILE="release-lto"

case "$CHANNEL" in
  oss)
    WARP_BIN="warp-oss"
    BINARY_NAME="warp-oss"
    APP_NAME="OpenWarp"
    BUNDLE_ID="dev.warp.OpenWarp"
    FEATURES="release_bundle,pprof_cpu_profiling"
    ;;
  dev)
    WARP_BIN="dev"
    BINARY_NAME="warp-dev"
    APP_NAME="WarpDev"
    BUNDLE_ID="dev.warp.WarpDev"
    FEATURES="release_bundle,crash_reporting,agent_mode_debug"
    ;;
  local)
    WARP_BIN="warp"
    BINARY_NAME="warp-local"
    APP_NAME="WarpLocal"
    BUNDLE_ID="dev.warp.WarpLocal"
    FEATURES="release_bundle,crash_reporting,agent_mode_debug"
    ;;
  *)
    echo "Unknown channel: $CHANNEL (use oss, dev, or local)"
    exit 1
    ;;
esac

echo "=== Installing build dependencies ==="
"$WORKSPACE_ROOT_DIR/script/linux/install_runtime_deps"

echo ""
echo "=== Building Warp ($CHANNEL channel) ==="
echo "  Binary: $WARP_BIN"
echo "  Profile: $CARGO_PROFILE"
echo "  Features: $FEATURES"

cargo build -p warp --profile "$CARGO_PROFILE" --bin "$WARP_BIN" --features "$FEATURES"

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WORKSPACE_ROOT_DIR/target}"
EXECUTABLE_PATH="$CARGO_TARGET_DIR/$CARGO_PROFILE/$WARP_BIN"

echo ""
echo "=== Installing to system ==="

# Install binary
sudo install -D "$EXECUTABLE_PATH" "/usr/local/bin/$BINARY_NAME"
echo "  Installed binary: /usr/local/bin/$BINARY_NAME"

# Install .desktop file
DESKTOP_SRC="$WORKSPACE_ROOT_DIR/app/channels/$CHANNEL/$BUNDLE_ID.desktop"
if [ -f "$DESKTOP_SRC" ]; then
  # Fix the Exec line to point to the actual binary path
  sudo install -Dm644 "$DESKTOP_SRC" "/usr/share/applications/$BUNDLE_ID.desktop"
  echo "  Installed desktop entry: /usr/share/applications/$BUNDLE_ID.desktop"
fi

# Install icons
ICON_DIR="$WORKSPACE_ROOT_DIR/app/channels/$CHANNEL/icon/no-padding"
if [ -d "$ICON_DIR" ]; then
  for icon in "$ICON_DIR"/*.png; do
    size="$(basename "$icon" .png)"
    sudo install -Dm644 "$icon" "/usr/share/icons/hicolor/${size}/apps/$BUNDLE_ID.png"
  done
  echo "  Installed icons"
fi

echo ""
echo "=== Build & install complete ==="
echo ""
echo "Run 'warp-oss' (or add WARP_FORCE_SOFTWARE=1 for CPU rendering)"
echo ""
echo "You may need to run: sudo update-desktop-database"
echo "And:                sudo update-icon-caches /usr/share/icons/*"
