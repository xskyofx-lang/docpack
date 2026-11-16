#!/usr/bin/env bash
# DocPack installer for macOS and Linux.
#   curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent codex
# Downloads the prebuilt binary into ~/.local/bin/docpack. With --agent
# (codex, kimi, antigravity, deepseek) it also installs that agent's skill and
# registers the `docpack mcp` server in its configuration (a copy of the old
# configuration is kept next to it). Safe to run again.
set -euo pipefail

REPO="xskyofx-lang/docpack"
RAW="https://raw.githubusercontent.com/${REPO}/main"
BIN_DIR="${HOME}/.local/bin"
AGENT=""
while [ $# -gt 0 ]; do
  case "$1" in
    --agent) AGENT="${2:-}"; shift 2 ;;
    --agent=*) AGENT="${1#*=}"; shift ;;
    *) echo "DocPack: opción desconocida: $1" >&2; exit 2 ;;
  esac
done
case "${AGENT}" in ""|codex|kimi|antigravity|deepseek) ;; *) echo "DocPack: --agent es codex, kimi, antigravity o deepseek" >&2; exit 2 ;; esac

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)               PLATFORM="linux-x64" ;;
  Linux-aarch64|Linux-arm64)  PLATFORM="linux-arm64" ;;
  Darwin-arm64)               PLATFORM="macos-arm64" ;;
  Darwin-x86_64)              PLATFORM="macos-x64" ;;
  *) echo "DocPack: plataforma no soportada: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

mkdir -p "${BIN_DIR}"
echo "DocPack: descargando ${PLATFORM} ..."
curl -fsSL "${RAW}/plugins/claude-code/native/${PLATFORM}/docpack" -o "${BIN_DIR}/docpack.download"
chmod +x "${BIN_DIR}/docpack.download"
mv -f "${BIN_DIR}/docpack.download" "${BIN_DIR}/docpack"
"${BIN_DIR}/docpack" --version
BIN="${BIN_DIR}/docpack"

case ":${PATH}:" in
  *":${BIN_DIR}:"*) ;;
  *) echo "Nota: ${BIN_DIR} no está en tu PATH. Agrega a tu shell: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
esac

[ -z "${AGENT}" ] && { echo "DocPack instalado en ${BIN}"; exit 0; }

# Skill: one folder per skill with its SKILL.md, where each agent looks for it.
case "${AGENT}" in
  antigravity) SKILL_DIR="${HOME}/.gemini/config/skills/docpack" ;;
  *)           SKILL_DIR="${HOME}/.agents/skills/docpack" ;;
esac
mkdir -p "${SKILL_DIR}"
curl -fsSL "${RAW}/plugins/${AGENT}/SKILL.md" -o "${SKILL_DIR}/SKILL.md"
echo "Skill instalada en ${SKILL_DIR}/SKILL.md"

# Adds docpack to the "mcpServers" of a JSON configuration, keeping the rest.
register_json() {
  local file="$1"
  mkdir -p "$(dirname "${file}")"
  [ -f "${file}" ] && cp "${file}" "${file}.antes-de-docpack"
  local merge='import json,os,sys
path,binary=sys.argv[1],sys.argv[2]
data=json.load(open(path,encoding="utf-8")) if os.path.isfile(path) and os.path.getsize(path) else {}
data.setdefault("mcpServers",{})["docpack"]={"command":binary,"args":["mcp"]}
open(path,"w",encoding="utf-8").write(json.dumps(data,indent=2,ensure_ascii=False)+"\n")'
  if command -v python3 >/dev/null 2>&1; then python3 -c "${merge}" "${file}" "${BIN}"
  elif command -v node >/dev/null 2>&1; then
    node -e 'const fs=require("fs");const [f,b]=process.argv.slice(1);let d={};try{d=JSON.parse(fs.readFileSync(f,"utf8")||"{}")}catch(e){if(e.code!=="ENOENT")throw e}(d.mcpServers??={}).docpack={command:b,args:["mcp"]};fs.writeFileSync(f,JSON.stringify(d,null,2)+"\n")' "${file}" "${BIN}"
  else
    echo "Sin python3 ni node para editar ${file}. Agrega a mano dentro de \"mcpServers\":"
    echo "  \"docpack\": { \"command\": \"${BIN}\", \"args\": [\"mcp\"] }"
    return
  fi
  echo "Servidor MCP docpack registrado en ${file}"
}

case "${AGENT}" in
  codex)
    if command -v codex >/dev/null 2>&1; then
      codex mcp remove docpack >/dev/null 2>&1 || true
      codex mcp add docpack -- "${BIN}" mcp
      echo "Servidor MCP docpack registrado en Codex (codex mcp list para verlo)"
    else
      echo "No encontré el comando codex. Cuando lo tengas: codex mcp add docpack -- \"${BIN}\" mcp"
    fi ;;
  kimi)        register_json "${KIMI_CODE_HOME:-${HOME}/.kimi-code}/mcp.json" ;;
  antigravity) register_json "${HOME}/.gemini/config/mcp_config.json" ;;
  deepseek)
    echo "DeepSeek no tiene un archivo de configuración único: cada cliente (DeepSeek-TUI, OpenCode, Cline…) tiene el suyo."
    echo "Agrega este servidor en la configuración MCP de tu cliente:"
    echo "  \"docpack\": { \"command\": \"${BIN}\", \"args\": [\"mcp\"] }" ;;
esac
echo "Listo. Reinicia ${AGENT} para que cargue la skill y el servidor MCP."
