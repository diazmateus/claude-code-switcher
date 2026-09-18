//! Registro de contas e estado ativo.
//!
//! Formato do accounts.conf (mesmo lido pelos shims de shell):
//!     nome=/caminho/do/config/dir
//!
//! A troca de conta escreve apenas o arquivo `active`. Nada de processo em
//! andamento e' tocado: o Claude Code le CLAUDE_CONFIG_DIR uma unica vez, ao
//! subir, e cada conta guarda credenciais no proprio dir.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Account {
    pub name: String,
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct AccountInfo {
    pub email: Option<String>,
    pub plan: Option<String>,
    /// Token expirado segundo o proprio arquivo de credenciais.
    pub token_expired: bool,
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn root() -> PathBuf {
    match std::env::var_os("CLAUDE_ACCOUNTS_HOME") {
        Some(v) => PathBuf::from(v),
        None => home().join(".claude-accounts"),
    }
}

pub fn conf_path() -> PathBuf { root().join("accounts.conf") }
pub fn active_path() -> PathBuf { root().join("active") }
pub fn bin_dir() -> PathBuf { root().join("bin") }
pub fn config_path() -> PathBuf { root().join("config") }

/// Opcoes simples em formato chave=valor.
pub fn get_flag(key: &str) -> bool {
    let Ok(text) = fs::read_to_string(config_path()) else { return false };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return matches!(v.trim(), "1" | "true" | "sim" | "yes");
            }
        }
    }
    false
}

pub fn set_flag(key: &str, on: bool) -> std::io::Result<()> {
    fs::create_dir_all(root())?;
    let text = fs::read_to_string(config_path()).unwrap_or_default();
    let mut out: Vec<String> = text
        .lines()
        .filter(|l| !l.split('#').next().unwrap_or("").trim().starts_with(&format!("{key}=")))
        .map(str::to_string)
        .collect();
    out.push(format!("{key}={}", if on { "true" } else { "false" }));
    fs::write(config_path(), out.join("\n") + "\n")
}

fn expand(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(raw)
    }
}

pub fn load() -> Vec<Account> {
    let text = match fs::read_to_string(conf_path()) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some((name, dir)) = line.split_once('=') {
            let name = name.trim();
            let dir = dir.trim();
            if !name.is_empty() && !dir.is_empty() {
                out.push(Account { name: name.to_string(), dir: expand(dir) });
            }
        }
    }
    out
}

pub fn find(name: &str) -> Option<Account> {
    load().into_iter().find(|a| a.name == name)
}

pub fn active_name() -> Option<String> {
    fs::read_to_string(active_path())
        .ok()
        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
        .filter(|s| !s.is_empty())
}

/// Troca a conta usada por sessoes novas. Grava de forma atomica para que um
/// shim lendo o arquivo ao mesmo tempo nunca veja conteudo pela metade.
pub fn set_active(name: &str) -> std::io::Result<()> {
    if find(name).is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("conta '{name}' nao registrada"),
        ));
    }
    let root = root();
    fs::create_dir_all(&root)?;
    let tmp = root.join("active.tmp");
    fs::write(&tmp, format!("{name}\n"))?;
    fs::rename(&tmp, active_path())
}

pub fn add(name: &str, dir: &Path) -> std::io::Result<()> {
    if find(name).is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("ja existe uma conta '{name}'"),
        ));
    }
    fs::create_dir_all(dir)?;
    fs::create_dir_all(root())?;
    let mut text = fs::read_to_string(conf_path()).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!("{name}={}\n", dir.display()));
    fs::write(conf_path(), text)
}

/// Le email, plano e validade do token direto dos arquivos da conta.
pub fn info(dir: &Path) -> AccountInfo {
    let mut info = AccountInfo::default();

    if let Ok(txt) = fs::read_to_string(dir.join(".claude.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
            info.email = v["oauthAccount"]["emailAddress"].as_str().map(str::to_string);
        }
    }
    if let Ok(txt) = fs::read_to_string(dir.join(".credentials.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
            let oauth = &v["claudeAiOauth"];
            info.plan = oauth["subscriptionType"].as_str().map(str::to_string);
            if let Some(ms) = oauth["expiresAt"].as_i64() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                info.token_expired = ms < now;
            }
        }
    }
    info
}

/// Rotulo curto para o menu: prefere o email, cai para o nome do dir.
pub fn label(acc: &Account, info: &AccountInfo) -> String {
    match &info.email {
        Some(e) => e.clone(),
        None => acc.dir.display().to_string(),
    }
}
