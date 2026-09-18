//! Descobre contas do Claude Code que já existem na máquina.
//!
//! Quem já usa duas contas quase sempre já tem os diretórios criados, seja por
//! `CLAUDE_CONFIG_DIR` na mão, seja por alias no shell. Pedir para registrar
//! tudo de novo seria pedir para repetir um trabalho já feito, e abriria espaço
//! para apontar para o diretório errado.

use crate::accounts::{self, Account};
use std::path::{Path, PathBuf};

pub struct Achado {
    pub nome: String,
    pub dir: PathBuf,
    pub email: Option<String>,
    /// Já está em accounts.conf.
    pub registrada: bool,
    /// Outro diretório já achado aponta para a mesma conta.
    pub duplicada_de: Option<String>,
}

/// Um diretório é conta do Claude Code se guarda credencial ou identidade.
fn eh_config_dir(p: &Path) -> bool {
    p.is_dir() && (p.join(".credentials.json").is_file() || p.join(".claude.json").is_file())
}

/// `.claude` vira "principal", `.claude-pessoal` vira "pessoal".
fn nome_sugerido(dir: &Path) -> String {
    let base = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let sem_ponto = base.trim_start_matches('.');
    match sem_ponto.strip_prefix("claude-") {
        Some(resto) if !resto.is_empty() => resto.to_string(),
        _ => "principal".to_string(),
    }
}

fn uuid_de(dir: &Path) -> Option<String> {
    let txt = std::fs::read_to_string(dir.join(".claude.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&txt).ok()?;
    v["oauthAccount"]["accountUuid"]
        .as_str()
        .map(str::to_string)
}

/// Varre o home atrás de diretórios de conta.
pub fn procurar() -> Vec<Achado> {
    let home = accounts::home();
    let registradas: Vec<Account> = accounts::load();

    let mut candidatos: Vec<PathBuf> = Vec::new();
    if let Ok(entradas) = std::fs::read_dir(&home) {
        for e in entradas.flatten() {
            let p = e.path();
            let nome = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if (nome == ".claude" || nome.starts_with(".claude-")) && eh_config_dir(&p) {
                candidatos.push(p);
            }
        }
    }
    // Um CLAUDE_CONFIG_DIR fora do padrão também conta.
    if let Some(v) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        let p = PathBuf::from(v);
        if eh_config_dir(&p) && !candidatos.contains(&p) {
            candidatos.push(p);
        }
    }
    candidatos.sort();

    let mut achados: Vec<Achado> = Vec::new();
    for dir in candidatos {
        let info = accounts::info(&dir);
        let uuid = uuid_de(&dir);
        // Mesma conta em dois diretórios: acontece quando alguém cria um dir
        // novo e faz login na mesma conta sem querer.
        let duplicada_de = uuid.as_ref().and_then(|u| {
            achados
                .iter()
                .find(|a| uuid_de(&a.dir).as_ref() == Some(u))
                .map(|a| a.nome.clone())
        });
        let registrada = registradas.iter().find(|a| a.dir == dir);
        achados.push(Achado {
            nome: registrada
                .map(|a| a.name.clone())
                .unwrap_or_else(|| nome_sugerido(&dir)),
            dir,
            email: info.email,
            registrada: registrada.is_some(),
            duplicada_de,
        });
    }
    achados
}

/// Registra o que ainda não está em accounts.conf, corrigindo o nome de cada
/// achado para o que foi de fato gravado. Devolve os nomes incluídos.
pub fn registrar(achados: &mut [Achado]) -> Vec<String> {
    let mut novos = Vec::new();
    for a in achados.iter_mut() {
        if a.registrada || a.duplicada_de.is_some() {
            continue;
        }
        // Nome já usado por outro diretório: desempata com um sufixo, e o
        // achado passa a carregar o nome real para não mentir na listagem.
        let mut nome = a.nome.clone();
        let mut n = 2;
        while accounts::find(&nome).is_some() {
            nome = format!("{}-{n}", a.nome);
            n += 1;
        }
        if accounts::add(&nome, &a.dir).is_ok() {
            a.nome = nome.clone();
            novos.push(nome);
        }
    }
    novos
}
