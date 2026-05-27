#!/usr/bin/env sh
set -eu

REPO="${DOSH_REPO:-duongonix/dosh}"
VERSION="${1:-latest}"
INSTALL_DIR="${DOSH_INSTALL_DIR:-$HOME/.local/bin}"
TMP_DIR="${TMPDIR:-/tmp}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: missing required command: $1" >&2
    exit 1
  }
}

need_cmd curl
need_cmd tar

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"

case "$os" in
  linux) os_name="linux" ;;
  darwin) os_name="macos" ;;
  *)
    echo "error: unsupported OS: $os" >&2
    exit 1
    ;;
esac

case "$arch" in
  x86_64|amd64) arch_name="x86_64" ;;
  aarch64|arm64) arch_name="aarch64" ;;
  *)
    echo "error: unsupported architecture: $arch" >&2
    exit 1
    ;;
esac

if [ "$VERSION" = "latest" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
fi

if [ -z "${VERSION:-}" ]; then
  echo "error: could not resolve release version. pass it explicitly: ./install.sh v1.0.0" >&2
  exit 1
fi

ASSET="dosh-${VERSION}-${os_name}-${arch_name}.tar.gz"
URL="https://github.com/$REPO/releases/download/$VERSION/$ASSET"
WORK_DIR="$TMP_DIR/dosh-install-$VERSION-$os_name-$arch_name-$$"

echo "Installing Dosh from $URL"
mkdir -p "$WORK_DIR" "$INSTALL_DIR"
trap 'rm -rf "$WORK_DIR"' EXIT INT TERM

curl -fL "$URL" -o "$WORK_DIR/$ASSET"
tar -xzf "$WORK_DIR/$ASSET" -C "$WORK_DIR"

PKG_DIR="$WORK_DIR/dosh-${VERSION}-${os_name}-${arch_name}"
if [ ! -d "$PKG_DIR" ]; then
  PKG_DIR="$(find "$WORK_DIR" -maxdepth 2 -type d -name "dosh-${VERSION}-${os_name}-${arch_name}" | head -n1)"
fi
if [ -z "${PKG_DIR:-}" ] || [ ! -d "$PKG_DIR" ]; then
  echo "error: package layout not recognized" >&2
  exit 1
fi

install -m 755 "$PKG_DIR/dosh" "$INSTALL_DIR/dosh"
if [ -f "$PKG_DIR/durl" ]; then
  install -m 755 "$PKG_DIR/durl" "$INSTALL_DIR/durl"
fi

echo "Installed:"
echo "  $INSTALL_DIR/dosh"
if [ -f "$INSTALL_DIR/durl" ]; then
  echo "  $INSTALL_DIR/durl"
fi

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo ""
    echo "Add to PATH (if needed):"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

echo ""
echo "Verify:"
echo "  dosh --version"

