#!/usr/bin/env bash
#
# Build and install Warp to the system.
#
# Usage:
#   ./script/build-and-install.sh [oss|dev|local]
#   ./script/build-and-install.sh --channel oss
#
# Environment variables:
#   WARP_FORCE_SOFTWARE=1   Force CPU software rendering (llvmpipe)
#   WARP_SOFTWARE_FPS=30    Cap FPS for software rendering (default 30)
#

set -euo pipefail

WORKSPACE_ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$WORKSPACE_ROOT_DIR"

if [[ "${1:-}" == "--channel" ]]; then
  CHANNEL="${2:-oss}"
elif [[ "${1:-}" == --channel=* ]]; then
  CHANNEL="${1#--channel=}"
else
  CHANNEL="${1:-oss}"
fi
CARGO_PROFILE="release-lto"

case "$CHANNEL" in
  oss)
    WARP_BIN="warp-oss"
    BINARY_NAME="warp-oss"
    DESKTOP_COMMAND="warp-terminal-oss"
    APP_NAME="OpenWarp"
    BUNDLE_ID="dev.warp.OpenWarp"
    FEATURES="release_bundle,gui,nld_improvements,pprof_cpu_profiling"
    ;;
  dev)
    WARP_BIN="dev"
    BINARY_NAME="warp-dev"
    DESKTOP_COMMAND="warp-terminal-dev"
    APP_NAME="WarpDev"
    BUNDLE_ID="dev.warp.WarpDev"
    FEATURES="release_bundle,gui,nld_improvements,crash_reporting,agent_mode_debug"
    ;;
  local)
    WARP_BIN="warp"
    BINARY_NAME="warp-local"
    DESKTOP_COMMAND="warp-terminal-local"
    APP_NAME="WarpLocal"
    BUNDLE_ID="dev.warp.WarpLocal"
    FEATURES="release_bundle,gui,nld_improvements,crash_reporting,agent_mode_debug"
    ;;
  *)
    echo "Unknown channel: $CHANNEL (use oss, dev, or local)"
    exit 1
    ;;
esac

echo "=== Installing build dependencies ==="
"$WORKSPACE_ROOT_DIR/script/linux/install_build_deps"
"$WORKSPACE_ROOT_DIR/script/linux/install_runtime_deps"

echo ""
echo "=== Building Warp ($CHANNEL channel) ==="
echo "  Binary: $WARP_BIN"
echo "  Profile: $CARGO_PROFILE"
echo "  Features: $FEATURES"

cargo build -p warp --profile "$CARGO_PROFILE" --bin "$WARP_BIN" --features "$FEATURES"

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WORKSPACE_ROOT_DIR/target}"
EXECUTABLE_PATH="$CARGO_TARGET_DIR/$CARGO_PROFILE/$WARP_BIN"
if [[ ! -x "$EXECUTABLE_PATH" ]]; then
  echo "Built executable not found: $EXECUTABLE_PATH"
  exit 1
fi

echo ""
echo "=== Installing to system ==="

# Install binary
sudo install -D "$EXECUTABLE_PATH" "/usr/local/bin/$DESKTOP_COMMAND"
sudo ln -sf "/usr/local/bin/$DESKTOP_COMMAND" "/usr/local/bin/$BINARY_NAME"
echo "  Installed binary: /usr/local/bin/$DESKTOP_COMMAND"
echo "  Installed compatibility symlink: /usr/local/bin/$BINARY_NAME"

# Install .desktop file
DESKTOP_SRC="$WORKSPACE_ROOT_DIR/app/channels/$CHANNEL/$BUNDLE_ID.desktop"
if [ -f "$DESKTOP_SRC" ]; then
  sudo install -Dm644 "$DESKTOP_SRC" "/usr/share/applications/$BUNDLE_ID.desktop"
  echo "  Installed desktop entry: /usr/share/applications/$BUNDLE_ID.desktop"
fi

# Install icons
ICON_DIR="$WORKSPACE_ROOT_DIR/app/channels/$CHANNEL/icon/no-padding"
if [ -d "$ICON_DIR" ]; then
  for icon in "$ICON_DIR"/*.png; do
    [ -e "$icon" ] || continue
    size="$(basename "$icon" .png)"
    sudo install -Dm644 "$icon" "/usr/share/icons/hicolor/${size}/apps/$BUNDLE_ID.png"
  done
  echo "  Installed icons"
fi

echo ""
echo "=== Build & install complete ==="
echo ""
echo "Run '$DESKTOP_COMMAND'"
echo ""
echo "You may need to run: sudo update-desktop-database"
echo "And:                sudo update-icon-caches /usr/share/icons/*"
