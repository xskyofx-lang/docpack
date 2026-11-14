#!/usr/bin/env bash
# DocPack installer for Muse (and any agent with a shell).
# Downloads the matching prebuilt DocPack binary into ~/.local/bin/docpack.
# No Rust, no Python, no build step. Idempotent: safe to run again.
set -euo pipefail

REPO="xskyofx-lang/docpack"
BRANCH="main"
BIN_DIR="${HOME}/.local/bin"

OS="$(uname -s)"
ARCH="$(uname -m)"
case "${OS}-${ARCH}" in
  Linux-x86_64)  PLATFORM="linux-x64" ;;
  Linux-aarch64) PLATFORM="linux-arm64" ;;
  Darwin-arm64)  PLATFORM="macos-arm64" ;;
  Darwin-x86_64) PLATFORM="macos-x64" ;;
  *) echo "DocPack: plataforma no soportada: ${OS}-${ARCH}" >&2; exit 1 ;;
esac

URL="https://raw.githubusercontent.com/${REPO}/${BRANCH}/plugins/claude-code/native/${PLATFORM}/docpack"
mkdir -p "${BIN_DIR}"
echo "DocPack: descargando ${PLATFORM} ..."
curl -fsSL "${URL}" -o "${BIN_DIR}/docpack"
chmod +x "${BIN_DIR}/docpack"
# Chequeo semanal de actualizaciones (lo instala junto al binario).
curl -fsSL "https://raw.githubusercontent.com/${REPO}/${BRANCH}/plugins/muse/check-update.sh" -o "${BIN_DIR}/check-update.sh"
chmod +x "${BIN_DIR}/check-update.sh"
"${BIN_DIR}/docpack" --version
echo "DocPack instalado en ${BIN_DIR}/docpack"
echo "Chequeo de actualizaciones en ${BIN_DIR}/check-update.sh (corre una vez por semana; si hay versión nueva te avisa con sus mejoras y te pregunta si la instalas)."
case ":${PATH}:" in
  *":${BIN_DIR}:"*) ;;
  *) echo "Nota: ${BIN_DIR} no está en tu PATH. Usa la ruta completa o agrega: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
esac
