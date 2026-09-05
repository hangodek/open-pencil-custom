#!/usr/bin/env bash
# Build and serve the Rust CanvasKit web editor without Docker.
#
# This is the script-publish counterpart to Dockerfile.web-rust. It builds the
# deployable wasm bundle, builds the GUI-free op-host-web-server daemon, then
# serves the browser editor through `--serve-web`.
set -euo pipefail

cd "$(dirname "$0")/.."

PORT="${OPENPENCIL_SERVE_PORT:-3100}"
HOST="${OPENPENCIL_SERVE_HOST:-127.0.0.1}"
FORCE_BUILD=0
OPEN_BROWSER=0
DOC_PATH=""

for arg in "$@"; do
  case "$arg" in
    --build|-b|--rebuild)
      FORCE_BUILD=1
      ;;
    --open|-o)
      OPEN_BROWSER=1
      ;;
    *)
      if [ -z "$DOC_PATH" ]; then
        DOC_PATH="$arg"
      fi
      ;;
  esac
done

WASM_FILE="crates/op-host-web/pkg/op_host_web_bg.wasm"
SERVER_BIN="${OPENPENCIL_WEB_SERVER_BIN:-target/release/op-host-web-server}"

# Build wasm bundle if missing or if rebuild requested
if [ ! -f "$WASM_FILE" ] || [ "$FORCE_BUILD" = "1" ] || [ "${OPENPENCIL_FORCE_BUILD:-0}" = "1" ]; then
  if [ "${OPENPENCIL_SKIP_WASM_BUILD:-0}" != "1" ]; then
    echo "Building WebAssembly CanvasKit bundle..."
    bash tools/check-wasm-bundle.sh
  fi
fi

# Build web server daemon if missing or if rebuild requested
if [ ! -x "$SERVER_BIN" ] || [ "$FORCE_BUILD" = "1" ] || [ "${OPENPENCIL_FORCE_BUILD:-0}" = "1" ]; then
  if [ "${OPENPENCIL_SKIP_SERVER_BUILD:-0}" != "1" ]; then
    echo "Building op-host-web-server binary..."
    cargo build -p op-host-web-server --release
  fi
fi

if [ ! -x "$SERVER_BIN" ]; then
  echo "error: server binary is not executable: $SERVER_BIN" >&2
  exit 1
fi

export OPENPENCIL_WEB_BUNDLE_DIR="${OPENPENCIL_WEB_BUNDLE_DIR:-$PWD/crates/op-host-web/pkg}"
export OPENPENCIL_CANVASKIT_DIR="${OPENPENCIL_CANVASKIT_DIR:-$PWD/crates/op-host-web/assets/canvaskit}"

URL="http://${HOST}:${PORT}/"
echo "OpenPencil Rust web editor: ${URL}"
echo "bundle: ${OPENPENCIL_WEB_BUNDLE_DIR}"
echo "canvaskit: ${OPENPENCIL_CANVASKIT_DIR}"

if [ "$OPEN_BROWSER" = "1" ]; then
  (
    sleep 1
    if command -v xdg-open >/dev/null 2>&1; then
      xdg-open "$URL" >/dev/null 2>&1 || true
    elif command -v open >/dev/null 2>&1; then
      open "$URL" >/dev/null 2>&1 || true
    fi
  ) &
fi

if [ -n "$DOC_PATH" ]; then
  exec "$SERVER_BIN" --serve-web "$PORT" "$DOC_PATH" --host "$HOST"
else
  exec "$SERVER_BIN" --serve-web "$PORT" --host "$HOST"
fi
