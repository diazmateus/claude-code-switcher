#!/bin/sh
# Instalador do ccswitch. Baixa o binário pronto do GitHub Releases.
#
#   curl -fsSL https://raw.githubusercontent.com/diazmateus/claude-code-switcher/main/install.sh | sh
#
# Não usa sudo e não escreve fora do seu diretório pessoal.
set -eu

REPO="diazmateus/claude-code-switcher"
DEST="${CCSWITCH_BIN_DIR:-$HOME/.local/bin}"

erro() { printf '\033[31merro:\033[0m %s\n' "$1" >&2; exit 1; }
info() { printf '  %s\n' "$1"; }

[ "$(uname -s)" = "Linux" ] || erro "por enquanto só há binário para Linux."
case "$(uname -m)" in
  x86_64|amd64) ALVO="x86_64-linux" ;;
  *) erro "sem binário para $(uname -m). Compile do fonte: https://github.com/$REPO" ;;
esac

if command -v curl >/dev/null 2>&1; then
  baixar() { curl -fsSL "$1" -o "$2"; }
  ler() { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
  baixar() { wget -qO "$2" "$1"; }
  ler() { wget -qO- "$1"; }
else
  erro "preciso de curl ou wget."
fi

printf '\nInstalando ccswitch\n\n'

VERSAO="${CCSWITCH_VERSION:-}"
if [ -z "$VERSAO" ]; then
  info "procurando a versão mais recente..."
  VERSAO=$(ler "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n1)
  [ -n "$VERSAO" ] || erro "não achei nenhuma release publicada ainda."
fi
info "versão: $VERSAO"

PACOTE="ccswitch-$ALVO.tar.gz"
BASE="https://github.com/$REPO/releases/download/$VERSAO"
TMP=$(mktemp -d)
# Sai limpo mesmo se algo falhar no meio.
trap 'rm -rf "$TMP"' EXIT INT TERM

info "baixando $PACOTE..."
baixar "$BASE/$PACOTE" "$TMP/$PACOTE" || erro "download falhou."

# Confere o checksum quando ele existe na release.
if baixar "$BASE/$PACOTE.sha256" "$TMP/$PACOTE.sha256" 2>/dev/null; then
  if command -v sha256sum >/dev/null 2>&1; then
    ESPERADO=$(cut -d' ' -f1 < "$TMP/$PACOTE.sha256")
    OBTIDO=$(sha256sum "$TMP/$PACOTE" | cut -d' ' -f1)
    [ "$ESPERADO" = "$OBTIDO" ] || erro "checksum não confere. Abortei."
    info "checksum confere"
  fi
fi

tar -xzf "$TMP/$PACOTE" -C "$TMP" || erro "não consegui extrair o pacote."
mkdir -p "$DEST"
install -m 755 "$TMP/ccswitch" "$DEST/ccswitch"
info "instalado em $DEST/ccswitch"

# ~/.local/bin nem sempre está no PATH em instalação nova.
case ":$PATH:" in
  *":$DEST:"*) ;;
  *) printf '\n  atenção: %s não está no seu PATH.\n' "$DEST"
     printf '  o próximo passo (ccswitch install) resolve isso.\n' ;;
esac

printf '\nPronto. Agora:\n\n'
printf '  %s/ccswitch detect     # encontra as contas que você já tem\n' "$DEST"
printf '  %s/ccswitch install    # coloca no PATH e inicia com o sistema\n\n' "$DEST"
