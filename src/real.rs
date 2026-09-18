//! Onde está o Claude Code de verdade.
//!
//! O shim precisa chamar o binário real, e ele mora em lugares diferentes
//! conforme o instalador. Um shim jamais pode chamar outro shim, então todo
//! candidato é conferido pela marca antes de ser aceito.

use std::path::{Path, PathBuf};

pub const MARCA: &str = "claude-accounts-shim";

/// Script de texto com a marca = é um shim nosso, não o Claude Code.
fn eh_shim(p: &Path) -> bool {
    let Ok(dados) = std::fs::read(p) else { return false };
    // Binário real é executável; shim é texto curto.
    if dados.len() > 64 * 1024 {
        return false;
    }
    String::from_utf8_lossy(&dados[..dados.len().min(2048)]).contains(MARCA)
}

fn aceita(p: PathBuf) -> Option<PathBuf> {
    if p.is_file() && !eh_shim(&p) { Some(p) } else { None }
}

/// Candidatos conhecidos, do mais provável ao menos.
fn candidatos() -> Vec<PathBuf> {
    let home = crate::accounts::home();
    let mut v = vec![
        // instalador oficial
        home.join(".local/bin/claude"),
        // instalações por gerenciador de pacotes do node
        home.join(".npm-global/bin/claude"),
        home.join(".bun/bin/claude"),
        home.join("node_modules/.bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
        PathBuf::from("/usr/bin/claude"),
    ];
    // prefixo customizado do npm, quando configurado
    if let Some(prefixo) = std::env::var_os("NPM_CONFIG_PREFIX") {
        v.push(PathBuf::from(prefixo).join("bin/claude"));
    }
    v
}

/// Versão mais recente sob `~/.local/share/claude/versions`, que é onde o
/// instalador oficial guarda os binários e para onde o link aponta.
fn versao_mais_recente() -> Option<PathBuf> {
    let base = crate::accounts::home().join(".local/share/claude/versions");
    let mut vs: Vec<_> = std::fs::read_dir(base)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    // Ordena por versão. Tem de ser file_name, não file_stem: em "2.1.276" o
    // ".276" passa por extensão e todas as versões viram "2.1".
    vs.sort_by_key(|p| {
        p.file_name()
            .and_then(|s| s.to_str())
            .map(|s| {
                s.split('.')
                    .filter_map(|n| n.parse::<u32>().ok())
                    .fold(0u64, |acc, n| acc * 100_000 + n as u64)
            })
            .unwrap_or(0)
    });
    vs.pop()
}

/// Procura no PATH, pulando qualquer shim.
fn no_path() -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        if let Some(p) = aceita(dir.join("claude")) {
            return Some(p);
        }
    }
    None
}

/// O binário real do Claude Code, ou `None` se não der para achar.
pub fn encontrar() -> Option<PathBuf> {
    candidatos()
        .into_iter()
        .find_map(aceita)
        .or_else(|| versao_mais_recente().and_then(aceita))
        .or_else(no_path)
}

/// Todos os lugares olhados e o que havia em cada um, para o diagnóstico.
pub fn diagnostico() -> Vec<(PathBuf, &'static str)> {
    let mut out = Vec::new();
    for c in candidatos() {
        let estado = if !c.exists() {
            "não existe"
        } else if eh_shim(&c) {
            "é o shim (ignorado)"
        } else {
            "ENCONTRADO"
        };
        out.push((c, estado));
    }
    if let Some(v) = versao_mais_recente() {
        let estado = if eh_shim(&v) { "é o shim (ignorado)" } else { "ENCONTRADO" };
        out.push((v, estado));
    }
    if let Some(p) = no_path() {
        out.push((p, "ENCONTRADO no PATH"));
    }
    out
}
