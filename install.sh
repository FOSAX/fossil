#!/bin/sh
# Installs the latest fossil release for the current platform.
#
#   curl -fsSL https://raw.githubusercontent.com/FOSAX/fossil/main/install.sh | sh
#
# Override destination with FOSSIL_INSTALL_DIR (default: $HOME/.local/bin).

set -eu

REPO="${FOSSIL_REPO:-FOSAX/fossil}"
INSTALL_DIR="${FOSSIL_INSTALL_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
    Linux) platform="unknown-linux-gnu" ;;
    Darwin) platform="apple-darwin" ;;
    *)
        echo "fossil: unsupported OS '$os'. Build from source instead: cargo build --release" >&2
        exit 1
        ;;
esac

case "$arch" in
    x86_64|amd64) cpu="x86_64" ;;
    arm64|aarch64) cpu="aarch64" ;;
    *)
        echo "fossil: unsupported architecture '$arch'. Build from source instead: cargo build --release" >&2
        exit 1
        ;;
esac

target="${cpu}-${platform}"
version="${FOSSIL_VERSION:-latest}"

if [ "$version" = "latest" ]; then
    url="https://github.com/${REPO}/releases/latest/download/fossil-${target}.tar.gz"
else
    url="https://github.com/${REPO}/releases/download/${version}/fossil-${target}.tar.gz"
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "fossil: downloading $url"
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$tmp_dir/fossil.tar.gz"
elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$tmp_dir/fossil.tar.gz"
else
    echo "fossil: need curl or wget to install" >&2
    exit 1
fi

tar xzf "$tmp_dir/fossil.tar.gz" -C "$tmp_dir"

mkdir -p "$INSTALL_DIR"
find "$tmp_dir" -type f -name fossil -exec cp {} "$INSTALL_DIR/fossil" \;
chmod +x "$INSTALL_DIR/fossil"

echo "fossil: installed to $INSTALL_DIR/fossil"
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) echo "fossil: add $INSTALL_DIR to your PATH to use 'fossil' directly" ;;
esac
