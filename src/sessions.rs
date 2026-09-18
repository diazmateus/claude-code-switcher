//! Descobre quais sessoes do Claude Code estao rodando e em qual conta.
//!
//! Serve para o menu poder avisar, antes da troca, que existem sessoes em
//! andamento - ainda que a troca nunca as afete.

use std::path::Path;

/// Quantas sessoes estao rodando neste config dir.
pub fn count_for(dir: &Path) -> usize {
    use std::fs;
    let default_dir = crate::accounts::home().join(".claude");
    let mut n = 0;

    let entries = match fs::read_dir("/proc") {
        Ok(e) => e,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        // So diretorios numericos: /proc/<pid>
        if !p
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()))
        {
            continue;
        }
        // O executavel precisa se chamar "claude".
        match fs::read_to_string(p.join("comm")) {
            Ok(c) if c.trim() == "claude" => {}
            _ => continue,
        }
        // environ e' NUL-separado; processo pode morrer no meio da leitura.
        let environ = match fs::read(p.join("environ")) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let mut found: Option<String> = None;
        for kv in environ.split(|b| *b == 0) {
            if let Some(v) = kv.strip_prefix(b"CLAUDE_CONFIG_DIR=") {
                found = Some(String::from_utf8_lossy(v).into_owned());
                break;
            }
        }
        // Sem a variavel, a sessao esta no dir default do Claude Code.
        let used = found
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| default_dir.clone());
        if used == dir {
            n += 1;
        }
    }
    n
}

/// No Windows nao da' para ler o ambiente de outro processo sem privilegios,
/// entao contamos o total de sessoes e o menu nao as atribui a uma conta.
#[cfg(target_os = "windows")]
pub fn count_for(_dir: &Path) -> usize {
    0
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn count_total() -> usize {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq claude.exe", "/NH", "/FO", "CSV"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| l.to_lowercase().contains("claude.exe"))
            .count(),
        Err(_) => 0,
    }
}

#[allow(dead_code)]
pub fn count_total() -> usize {
    crate::accounts::load()
        .iter()
        .map(|a| count_for(&a.dir))
        .sum()
}
