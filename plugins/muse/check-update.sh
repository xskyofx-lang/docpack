#!/usr/bin/env bash
# DocPack: chequeo semanal de actualizaciones.
# Compara la versión instalada con la publicada en la rama main del repo
# (plugins/claude-code/.claude-plugin/plugin.json, la misma que publica cada
# versión) y toma sus mejoras de CHANGELOG.md. No depende de que exista un
# release de GitHub. No instala nada. Salida pensada para que el agente la lea:
#   primera línea: STATUS=up_to_date | STATUS=update_available |
#                  STATUS=not_installed | STATUS=check_failed
# Si hay actualización, imprime también las mejoras de cada versión nueva.
set -u

REPO="xskyofx-lang/docpack"
RAW="https://raw.githubusercontent.com/${REPO}/main"
BIN="${HOME}/.local/bin/docpack"
[ -x "$BIN" ] || BIN="docpack"

INSTALLED="$("$BIN" --version 2>/dev/null | awk '{print $2}')"
if [ -z "$INSTALLED" ]; then
  echo "STATUS=not_installed"
  echo "DocPack no está instalado."
  exit 2
fi

PLUGIN="$(curl -fsSL --max-time 20 "${RAW}/plugins/claude-code/.claude-plugin/plugin.json" 2>/dev/null)" \
  || { echo "STATUS=check_failed"; echo "No se pudo consultar GitHub."; exit 3; }
LATEST="$(printf '%s' "$PLUGIN" | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
if [ -z "$LATEST" ]; then
  echo "STATUS=check_failed"
  echo "GitHub no devolvió una versión."
  exit 3
fi

if [ "$(printf '%s\n%s\n' "$INSTALLED" "$LATEST" | sort -V | tail -n 1)" = "$INSTALLED" ]; then
  echo "STATUS=up_to_date"
  echo "Instalado: $INSTALLED | Último: $LATEST"
  exit 0
fi

echo "STATUS=update_available"
echo "Instalado: $INSTALLED | Último: $LATEST"
# Las mejoras: las secciones "## <versión>" de CHANGELOG.md más nuevas que la
# instalada (el CHANGELOG va de la más nueva a la más vieja).
CHANGELOG="$(curl -fsSL --max-time 20 "${RAW}/CHANGELOG.md" 2>/dev/null)"
if [ -z "$CHANGELOG" ]; then
  echo "(No se pudieron leer las mejoras: https://github.com/${REPO}/blob/main/CHANGELOG.md)"
  exit 0
fi
printf '%s\n' "$CHANGELOG" | awk -v installed="## ${INSTALLED}" '
  $0 == installed { exit }
  /^## / { show = 1; print "--- Mejoras de la versión " substr($0, 4) " ---"; next }
  show { print }
' | head -n 80
