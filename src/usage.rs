//! Uso por conta, lido dos transcripts que a própria conta guarda.
//!
//! Fonte: `<config dir>/projects/**/*.jsonl`, que cada conta escreve no seu
//! próprio diretório. Nada de rede, nada de cota gasta, nenhuma dependência de
//! outro sistema, e some sozinho se a conta for removida.
//!
//! O que sai daqui é TOKENS CONSUMIDOS, não o percentual do limite: esse número
//! só existe nos headers de resposta da API e não é gravado em lugar nenhum.

use chrono::{DateTime, Duration, Utc};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct Usage {
    /// Início e fim do ciclo de 7 dias em curso.
    pub semana_inicio: Option<DateTime<Utc>>,
    pub semana_fim: Option<DateTime<Utc>>,
    /// Início do bloco de 5 horas em curso.
    pub sessao_inicio: Option<DateTime<Utc>>,
    /// Tokens no bloco de 5 horas em curso.
    pub sessao_tokens: u64,
    pub sessao_chamadas: u64,
    /// Quando o bloco de 5 horas atual termina.
    pub sessao_fim: Option<DateTime<Utc>>,
    /// Tokens nos últimos 7 dias.
    pub semana_tokens: u64,
    #[allow(dead_code)]
    pub semana_chamadas: u64,
}

/// Peso do modelo, relativo ao input do Opus (preço de tabela).
/// Sem isto, 1M de Haiku parece o mesmo que 1M de Opus, que custa 15x mais.
fn peso_modelo(modelo: &str) -> f64 {
    let m = modelo.to_ascii_lowercase();
    if m.contains("opus") {
        1.0
    } else if m.contains("sonnet") {
        1.0 / 5.0
    } else if m.contains("haiku") {
        1.0 / 15.0
    } else {
        1.0
    }
}

/// Tokens equivalentes: cada tipo entra pelo que ele pesa, não pelo volume
/// bruto. Cache lido custa 10% do input e domina o volume, e somá-lo inteiro
/// infla o número várias vezes e dá a impressão de um consumo que não houve.
fn soma_tokens(u: &serde_json::Value, modelo: &str) -> u64 {
    let g = |k: &str| u[k].as_u64().unwrap_or(0) as f64;
    let bruto = g("input_tokens")
        + g("output_tokens") * 5.0
        + g("cache_creation_input_tokens") * 1.25
        + g("cache_read_input_tokens") * 0.1;
    (bruto * peso_modelo(modelo)).round() as u64
}

/// Momento mais antigo com atividade na conta, pelos metadados dos arquivos.
/// Serve de âncora do ciclo semanal; ler só mtime é barato mesmo com milhares
/// de arquivos.
fn primeiro_uso(projects: &Path) -> Option<DateTime<Utc>> {
    let mut menor: Option<std::time::SystemTime> = None;
    let dirs = std::fs::read_dir(projects).ok()?;
    for d in dirs.flatten() {
        let Ok(arquivos) = std::fs::read_dir(d.path()) else {
            continue;
        };
        for f in arquivos.flatten() {
            if f.path().extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            if let Ok(m) = f.metadata().and_then(|m| m.modified()) {
                menor = Some(match menor {
                    Some(atual) if atual <= m => atual,
                    _ => m,
                });
            }
        }
    }
    menor.map(DateTime::<Utc>::from)
}

/// A âncora do ciclo é gravada na primeira vez e reusada depois. Sem isso, um
/// arquivo antigo tocado por acaso moveria a âncora e o "dia N/7" pularia.
fn ancora(dir: &Path, projects: &Path, agora: DateTime<Utc>) -> DateTime<Utc> {
    let chave = dir
        .file_name()
        .map(|s| s.to_string_lossy().trim_start_matches('.').to_string())
        .unwrap_or_else(|| "conta".into());
    let cache = crate::accounts::root().join(format!("ciclo-{chave}"));

    if let Ok(txt) = std::fs::read_to_string(&cache)
        && let Ok(t) = txt.trim().parse::<DateTime<Utc>>()
    {
        return t;
    }
    let t = primeiro_uso(projects).unwrap_or(agora);
    let _ = std::fs::create_dir_all(crate::accounts::root());
    let _ = std::fs::write(&cache, t.to_rfc3339());
    t
}

/// `janelas` = (início da janela de 5h, início do ciclo de 7 dias) quando a API
/// já informou os resets verdadeiros. Sem isso, as janelas são estimadas a
/// partir dos próprios transcripts.
pub fn compute(dir: &Path, janelas: Option<(DateTime<Utc>, DateTime<Utc>)>) -> Usage {
    let agora = Utc::now();
    let projects_dir = dir.join("projects");

    // O ciclo de 7 dias é ancorado no primeiro uso da conta e avança de 7 em 7.
    // A Anthropic não expõe quando o ciclo dela vira, então isto é a melhor
    // aproximação possível só com dados locais.
    let (semana_inicio, semana_fim) = match janelas {
        Some((_, ini7)) => (ini7, ini7 + Duration::days(7)),
        None => {
            let ancora = ancora(dir, &projects_dir, agora);
            let ciclos = ((agora - ancora).num_seconds() / Duration::days(7).num_seconds()).max(0);
            let ini = ancora + Duration::days(7 * ciclos);
            (ini, ini + Duration::days(7))
        }
    };
    let corte = semana_inicio;
    let corte_sys: std::time::SystemTime = corte.into();

    let projects = projects_dir;
    let mut eventos: Vec<(DateTime<Utc>, u64)> = Vec::new();

    let Ok(dirs) = std::fs::read_dir(&projects) else {
        return Usage::default();
    };
    for d in dirs.flatten() {
        let Ok(arquivos) = std::fs::read_dir(d.path()) else {
            continue;
        };
        for f in arquivos.flatten() {
            let p = f.path();
            if p.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            // Arquivo parado há mais de 7 dias não tem nada da janela.
            match f.metadata().and_then(|m| m.modified()) {
                Ok(m) if m >= corte_sys => {}
                _ => continue,
            }
            let Ok(file) = File::open(&p) else { continue };
            for linha in BufReader::new(file).lines().map_while(Result::ok) {
                // Filtro barato antes de pagar o parse de JSON.
                if !linha.contains("\"usage\"") {
                    continue;
                }
                let Ok(o) = serde_json::from_str::<serde_json::Value>(&linha) else {
                    continue;
                };
                let u = &o["message"]["usage"];
                if !u.is_object() {
                    continue;
                }
                let Some(ts) = o["timestamp"].as_str() else {
                    continue;
                };
                let Ok(t) = ts.parse::<DateTime<Utc>>() else {
                    continue;
                };
                if t < corte {
                    continue;
                }
                let modelo = o["message"]["model"].as_str().unwrap_or("");
                eventos.push((t, soma_tokens(u, modelo)));
            }
        }
    }

    if eventos.is_empty() {
        return Usage {
            semana_inicio: Some(semana_inicio),
            semana_fim: Some(semana_fim),
            ..Default::default()
        };
    }
    eventos.sort_by_key(|e| e.0);

    // O bloco de 5h não é "as últimas 5 horas": ele nasce na primeira chamada e
    // dura 5 horas. Blocos seguintes começam na primeira chamada após o fim.
    let (inicio, fim) = match janelas {
        Some((ini5, _)) => (ini5, ini5 + Duration::hours(5)),
        None => {
            let mut ini = eventos[0].0;
            for (t, _) in &eventos {
                if *t >= ini + Duration::hours(5) {
                    ini = *t;
                }
            }
            (ini, ini + Duration::hours(5))
        }
    };

    let mut u = Usage {
        semana_inicio: Some(semana_inicio),
        semana_fim: Some(semana_fim),
        semana_chamadas: eventos.len() as u64,
        semana_tokens: eventos.iter().map(|e| e.1).sum(),
        ..Default::default()
    };
    // Bloco expirado não conta como sessão em curso.
    if agora < fim {
        u.sessao_inicio = Some(inicio);
        u.sessao_fim = Some(fim);
        for (t, n) in &eventos {
            if *t >= inicio {
                u.sessao_tokens += n;
                u.sessao_chamadas += 1;
            }
        }
    }
    u
}

/// 1_234_567 -> "1,2M"
pub fn fmt_tokens(n: u64) -> String {
    match n {
        0 => "0".into(),
        n if n < 1_000 => format!("{n}"),
        n if n < 1_000_000 => format!("{:.0}k", n as f64 / 1e3),
        n if n < 1_000_000_000 => format!("{:.1}M", n as f64 / 1e6).replace('.', ","),
        n => format!("{:.1}B", n as f64 / 1e9).replace('.', ","),
    }
}

/// Barra de progresso em blocos. `fracao` fora de 0..1 é grampeada.
pub fn barra(fracao: f64, largura: usize) -> String {
    let f = fracao.clamp(0.0, 1.0);
    let cheios = (f * largura as f64).round() as usize;
    let mut s = String::with_capacity(largura * 3);
    for i in 0..largura {
        s.push(if i < cheios { '█' } else { '░' });
    }
    s
}

impl Usage {
    /// Quanto da janela de 5 horas já correu.
    pub fn sessao_fracao(&self, agora: DateTime<Utc>) -> Option<f64> {
        let (ini, fim) = (self.sessao_inicio?, self.sessao_fim?);
        let total = (fim - ini).num_seconds() as f64;
        if total <= 0.0 {
            return None;
        }
        Some(((agora - ini).num_seconds() as f64 / total).clamp(0.0, 1.0))
    }

    /// Quanto do ciclo de 7 dias já correu, e em que dia ele está.
    pub fn semana_fracao(&self, agora: DateTime<Utc>) -> Option<(f64, i64)> {
        let (ini, fim) = (self.semana_inicio?, self.semana_fim?);
        let total = (fim - ini).num_seconds() as f64;
        if total <= 0.0 {
            return None;
        }
        let f = ((agora - ini).num_seconds() as f64 / total).clamp(0.0, 1.0);
        let dia = ((agora - ini).num_days() + 1).clamp(1, 7);
        Some((f, dia))
    }
}
