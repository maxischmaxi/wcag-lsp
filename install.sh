#!/bin/sh
set -e

REPO="maxischmaxi/wcag-lsp"
BINARY="wcag-lsp"
INSTALL_DIR="${WCAG_LSP_INSTALL_DIR:-$HOME/.local/bin}"

get_target() {
    os=$(uname -s)
    arch=$(uname -m)

    case "$os" in
        Linux)  os_part="unknown-linux-musl" ;;
        Darwin) os_part="apple-darwin" ;;
        *)      echo "Error: unsupported OS: $os" >&2; exit 1 ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch_part="x86_64" ;;
        aarch64|arm64) arch_part="aarch64" ;;
        *)             echo "Error: unsupported architecture: $arch" >&2; exit 1 ;;
    esac

    echo "${arch_part}-${os_part}"
}

get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed 's/.*"tag_name": *"//;s/".*//'
}

main() {
    target=$(get_target)
    version="${1:-$(get_latest_version)}"

    if [ -z "$version" ]; then
        echo "Error: could not determine latest version" >&2
        exit 1
    fi

    url="https://github.com/${REPO}/releases/download/${version}/${BINARY}-${target}.tar.gz"

    echo "Installing ${BINARY} ${version} (${target})..."
    echo "  -> ${url}"

    mkdir -p "$INSTALL_DIR"

    curl -fsSL "$url" | tar xz -C "$INSTALL_DIR"
    chmod +x "${INSTALL_DIR}/${BINARY}"

    echo "Installed to ${INSTALL_DIR}/${BINARY}"

    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        echo ""
        echo "Add this to your shell profile:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    fi
}

main "$@"
