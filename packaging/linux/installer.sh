#!/bin/sh
# Installe Varium pour l'utilisateur courant (sans droits administrateur) :
# binaire dans ~/.local/bin, lanceur et icône, types de fichiers — un double-clic
# sur un document, un classeur ou une présentation l'ouvre dans Varium, et
# « Ouvrir avec » propose Varium pour .docx, .odt, .xlsx, .ods, .csv, .odp, .pptx.
#
#   ./installer.sh            installer
#   ./installer.sh --retirer  désinstaller
set -eu
ici=$(cd "$(dirname "$0")" && pwd)
donnees=${XDG_DATA_HOME:-$HOME/.local/share}
bin=$HOME/.local/bin

if [ "${1:-}" = "--retirer" ]; then
  rm -f "$bin/varium" "$donnees/applications/varium.desktop" \
    "$donnees/icons/hicolor/256x256/apps/varium.png" \
    "$donnees/icons/hicolor/scalable/apps/varium.svg" \
    "$donnees/mime/packages/varium-mime.xml"
else
  install -Dm755 "$ici/varium" "$bin/varium"
  install -Dm644 "$ici/varium.desktop" "$donnees/applications/varium.desktop"
  install -Dm644 "$ici/varium.png" "$donnees/icons/hicolor/256x256/apps/varium.png"
  install -Dm644 "$ici/varium.svg" "$donnees/icons/hicolor/scalable/apps/varium.svg"
  install -Dm644 "$ici/varium-mime.xml" "$donnees/mime/packages/varium-mime.xml"
  # Varium par défaut pour ses propres fichiers.
  if command -v xdg-mime >/dev/null 2>&1; then
    xdg-mime default varium.desktop application/x-varium application/x-varium-calc application/x-varium-slides || true
  fi
fi
command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$donnees/mime" || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$donnees/applications" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$donnees/icons/hicolor" || true
echo "Varium : terminé."
