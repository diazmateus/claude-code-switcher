//! Abrir uma sessao do Claude Code numa conta, a partir do menu.

use crate::accounts::Account;
use std::process::Command;

/// Terminais mais comuns, do mais provavel ao mais generico.
const TERMINALS: &[(&str, &[&str])] = &[
    ("ptyxis", &["--"]),
    ("gnome-terminal", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("kitty", &[]),
    ("alacritty", &["-e"]),
    ("wezterm", &["start", "--"]),
    ("x-terminal-emulator", &["-e"]),
    ("xterm", &["-e"]),
];

/// Abre um terminal novo ja' dentro da conta pedida.
/// `extra` sao argumentos passados ao claude (ex.: ["/login"]).
pub fn open_session(acc: &Account, extra: &[&str]) {
    let mut inner = String::from("claude");
    for e in extra {
        inner.push(' ');
        inner.push_str(e);
    }
    // Mantem o shell aberto depois da sessao, para a pessoa ler qualquer erro.
    let script = format!(
        "export CLAUDE_CONFIG_DIR={dir:?}; export CLAUDE_ACCOUNT={name:?}; \
         {inner}; echo; echo '[sessao encerrada - enter para fechar]'; read _",
        dir = acc.dir.display().to_string(),
        name = acc.name,
    );

    for (term, args) in TERMINALS {
        if which(term).is_none() {
            continue;
        }
        let mut cmd = Command::new(term);
        cmd.args(*args).arg("bash").arg("-lc").arg(&script);
        if cmd.spawn().is_ok() {
            return;
        }
    }
    eprintln!("nenhum terminal conhecido encontrado para abrir a sessao");
}

pub fn which(prog: &str) -> Option<std::path::PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let p = dir.join(prog);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}
