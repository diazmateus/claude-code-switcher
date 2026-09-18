//! Instalacao automatica: shims no PATH + inicio junto com o sistema.
//!
//! O shim e' o que faz a troca valer para tudo (terminal, Cursor, VS Code,
//! scripts), e nao so' para o shell interativo como um alias faria. Ele resolve
//! a conta no instante em que o processo sobe, entao trocar a conta nunca
//! alcanca uma sessao ja em andamento.

use crate::accounts;
use std::fs;
use std::io;

pub struct Report(pub Vec<String>);

impl Report {
    fn say(&mut self, s: impl Into<String>) { self.0.push(s.into()); }
}

const SHIM_SH: &str = r##"#!/usr/bin/env bash
# claude-accounts-shim v2  <- marca: nao remover
# Resolve a conta no instante do lancamento. Sessoes ja rodando mantem o
# CLAUDE_CONFIG_DIR que capturaram ao subir.
ACCT_HOME="${CLAUDE_ACCOUNTS_HOME:-$HOME/.claude-accounts}"

_conta_padrao() {
  [ -r "$ACCT_HOME/active" ] || return
  read -r _name < "$ACCT_HOME/active" || return
  [ -n "$_name" ] && [ -r "$ACCT_HOME/accounts.conf" ] || return
  while IFS='=' read -r _k _v; do
    case "$_k" in ''|\#*) continue ;; esac
    if [ "$_k" = "$_name" ]; then
      case "$_v" in "~/"*) _v="$HOME/${_v#\~/}" ;; esac
      if [ -d "$_v" ]; then
        export CLAUDE_CONFIG_DIR="$_v"
        export CLAUDE_ACCOUNT="$_name"
      else
        echo "claude: conta '$_name' aponta para '$_v', que nao existe." >&2
      fi
      break
    fi
  done < "$ACCT_HOME/accounts.conf"
}

if [ -z "${CLAUDE_CONFIG_DIR:-}" ]; then
  # Perguntar a conta so' faz sentido com terminal de verdade e em modo
  # interativo: script, pipe e -p seguem o padrao, calados.
  _ask=0
  if [ -r "$ACCT_HOME/config" ] && \
     grep -qiE '^[[:space:]]*ask_on_launch[[:space:]]*=[[:space:]]*(1|true|sim|yes)' "$ACCT_HOME/config"; then
    _ask=1
  fi
  for _a in "$@"; do
    case "$_a" in
      -p|--print|--output-format|-c|--continue|-r|--resume) _ask=0 ;;
    esac
  done
  { [ -t 0 ] && [ -t 2 ]; } || _ask=0

  if [ "$_ask" = 1 ]; then
    _cc="$HOME/.local/bin/ccswitch"
    if [ ! -x "$_cc" ]; then
      _cc="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/ccswitch"
    fi
    if [ -x "$_cc" ]; then
      _d="$("$_cc" pick)"
      if [ -n "$_d" ] && [ -d "$_d" ]; then
        export CLAUDE_CONFIG_DIR="$_d"
      fi
    fi
  fi

  [ -z "${CLAUDE_CONFIG_DIR:-}" ] && _conta_padrao
fi

# Um shim jamais pode chamar outro shim: candidatos com a marca sao descartados.
_is_shim() {
  [ -f "$1" ] || return 1
  [ "$(head -c 2 "$1" 2>/dev/null)" = "#!" ] || return 1
  head -n 3 "$1" 2>/dev/null | grep -q "claude-accounts-shim"
}

_real=""
if [ -x "$HOME/.local/bin/claude" ] && ! _is_shim "$HOME/.local/bin/claude"; then
  _real="$HOME/.local/bin/claude"
fi
if [ -z "$_real" ]; then
  _v="$(ls -1 "$HOME/.local/share/claude/versions" 2>/dev/null | sort -V | tail -n1)"
  [ -n "$_v" ] && [ -x "$HOME/.local/share/claude/versions/$_v" ] \
    && _real="$HOME/.local/share/claude/versions/$_v"
fi
if [ -z "$_real" ]; then
  while IFS= read -r _cand; do
    _is_shim "$_cand" && continue
    [ -x "$_cand" ] && { _real="$_cand"; break; }
  done < <(type -aP claude 2>/dev/null)
fi
if [ -z "$_real" ]; then
  echo "claude: binario real do Claude Code nao encontrado." >&2
  exit 127
fi

exec "$_real" "$@"
"##;

/// Escreve os shims da plataforma em ~/.claude-accounts/bin.
pub fn write_shims(r: &mut Report) -> io::Result<()> {
    let bin = accounts::bin_dir();
    fs::create_dir_all(&bin)?;

    let p = bin.join("claude");
    fs::write(&p, SHIM_SH)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755))?;
    }
    r.say(format!("shim claude em {}", p.display()));
    Ok(())
}

pub fn shim_installed() -> bool {
    accounts::bin_dir().join("claude").exists()
}

/// O shim esta a frente do claude real no PATH deste processo?
pub fn shim_wins_path() -> bool {
    let bin = accounts::bin_dir();
    let exe = "claude";
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            if dir == bin {
                return true;
            }
            if dir.join(exe).exists() {
                return false;
            }
        }
    }
    false
}


const MARK_BEGIN: &str = "# >>> claude-accounts (ccswitch) >>>";
const MARK_END: &str = "# <<< claude-accounts (ccswitch) <<<";

fn ensure_block(file: &std::path::Path, body: &str, r: &mut Report) -> io::Result<()> {
    let mut text = fs::read_to_string(file).unwrap_or_default();
    if text.contains(MARK_BEGIN) {
        // Ja instalado: substitui o bloco para refletir mudancas de caminho.
        if let (Some(a), Some(b)) = (text.find(MARK_BEGIN), text.find(MARK_END)) {
            let end = b + MARK_END.len();
            let new = format!("{MARK_BEGIN}\n{body}\n{MARK_END}");
            text.replace_range(a..end, &new);
            fs::write(file, text)?;
            r.say(format!("PATH atualizado em {}", file.display()));
        }
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!("\n{MARK_BEGIN}\n{body}\n{MARK_END}\n"));
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(file, text)?;
    r.say(format!("PATH instalado em {}", file.display()));
    Ok(())
}

/// Coloca o dir dos shims a frente do PATH, de forma persistente.
pub fn install_path(r: &mut Report) -> io::Result<()> {
    let bin = accounts::bin_dir();
    let body = format!("export PATH=\"{}:$PATH\"", bin.display());
    let home = accounts::home();

    // Shell interativo comum.
    let bashrc = home.join(".bashrc");
    if bashrc.exists() {
        ensure_block(&bashrc, &body, r)?;
    }

    // Shell de LOGIN: o bash lê só o primeiro destes que existir e ignora os
    // demais. Escrever no .profile quando existe um .bash_profile não tem
    // efeito nenhum - foi assim que a primeira versão disto falhou.
    let login = [".bash_profile", ".bash_login", ".profile"]
        .iter()
        .map(|n| home.join(n))
        .find(|p| p.exists())
        .unwrap_or_else(|| home.join(".profile"));
    ensure_block(&login, &body, r)?;
    if login.file_name().is_some_and(|n| n != ".profile") {
        r.say(format!(
            "{} é o arquivo de login em uso; o .profile seria ignorado",
            login.file_name().unwrap().to_string_lossy()
        ));
    }

    // Zsh, quando instalado.
    for name in [".zshrc", ".zprofile"] {
        let f = home.join(name);
        if f.exists() {
            ensure_block(&f, &body, r)?;
        }
    }

    // Aplicativos gráficos (Cursor, VS Code, atalhos do menu) não leem .bashrc.
    // A sessão de usuário do systemd lê environment.d e repassa o ambiente.
    let envd = home.join(".config/environment.d/50-claude-accounts.conf");
    fs::create_dir_all(envd.parent().unwrap())?;
    fs::write(&envd, format!("PATH={}:${{PATH}}\n", bin.display()))?;
    r.say(format!("PATH para apps gráficos em {}", envd.display()));
    r.say("apps já abertos (Cursor, VS Code) só enxergam o PATH novo depois de reiniciar".to_string());
    Ok(())
}

/// Inicia junto com a sessao grafica.
pub fn set_autostart(on: bool, r: &mut Report) -> io::Result<()> {
    let f = accounts::home().join(".config/autostart/claude-switcher.desktop");
    if !on {
        let _ = fs::remove_file(&f);
        r.say("nao inicia mais com o sistema".to_string());
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    fs::create_dir_all(f.parent().unwrap())?;
    fs::write(
        &f,
        format!(
            "[Desktop Entry]\nType=Application\nName=Claude Switcher\n\
             Comment=Troca a conta do Claude Code pela bandeja\n\
             Exec={} tray\nIcon=utilities-terminal\nTerminal=false\n\
             X-GNOME-Autostart-enabled=true\n",
            exe.display()
        ),
    )?;
    r.say("inicia junto com o sistema".to_string());
    Ok(())
}

pub fn autostart_enabled() -> bool {
    accounts::home().join(".config/autostart/claude-switcher.desktop").exists()
}

pub fn install_all(autostart: bool) -> io::Result<Report> {
    let mut r = Report(Vec::new());
    write_shims(&mut r)?;
    install_path(&mut r)?;

    if autostart {
        set_autostart(true, &mut r)?;
    }
    Ok(r)
}

