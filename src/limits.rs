//! Cota real das janelas, lida dos headers de rate limit da API.
//!
//! O percentual que o `/usage` mostra não é gravado em lugar nenhum: ele chega
//! nos headers `anthropic-ratelimit-unified-*` de cada resposta. Para tê-lo
//! fora de uma sessão, mandamos a menor requisição possível (Haiku, um token de
//! saída) e lemos só os cabeçalhos, e a resposta em si é descartada.
//!
//! O resultado é guardado em disco com validade, para que abrir o menu várias
//! vezes não vire várias chamadas.

use chrono::{DateTime, TimeZone, Utc};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// De quanto em quanto tempo vale a pena perguntar de novo.
const VALIDADE: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone)]
pub struct Limits {
    /// 0.0 a 1.0, a mesma fração que o /usage mostra.
    pub uso_5h: f64,
    pub uso_7d: f64,
    pub reset_5h: Option<DateTime<Utc>>,
    pub reset_7d: Option<DateTime<Utc>>,
    /// "allowed", "rejected"…
    pub status: String,
    pub obtido_em: DateTime<Utc>,
}

fn token(dir: &Path) -> Option<String> {
    let txt = std::fs::read_to_string(dir.join(".credentials.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&txt).ok()?;
    v["claudeAiOauth"]["accessToken"]
        .as_str()
        .map(str::to_string)
}

fn cache_path(dir: &Path) -> std::path::PathBuf {
    let chave = dir
        .file_name()
        .map(|s| s.to_string_lossy().trim_start_matches('.').to_string())
        .unwrap_or_else(|| "conta".into());
    crate::accounts::root().join(format!("cota-{chave}.json"))
}

fn ler_cache(dir: &Path, aceitar_velho: bool) -> Option<Limits> {
    let p = cache_path(dir);
    let meta = std::fs::metadata(&p).ok()?;
    let idade = SystemTime::now()
        .duration_since(meta.modified().ok()?)
        .ok()?;
    if !aceitar_velho && idade > VALIDADE {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?;
    Some(Limits {
        uso_5h: v["uso_5h"].as_f64()?,
        uso_7d: v["uso_7d"].as_f64()?,
        reset_5h: v["reset_5h"]
            .as_i64()
            .and_then(|s| Utc.timestamp_opt(s, 0).single()),
        reset_7d: v["reset_7d"]
            .as_i64()
            .and_then(|s| Utc.timestamp_opt(s, 0).single()),
        status: v["status"].as_str().unwrap_or("").to_string(),
        obtido_em: v["obtido_em"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
    })
}

fn gravar_cache(dir: &Path, l: &Limits) {
    let v = serde_json::json!({
        "uso_5h": l.uso_5h,
        "uso_7d": l.uso_7d,
        "reset_5h": l.reset_5h.map(|t| t.timestamp()),
        "reset_7d": l.reset_7d.map(|t| t.timestamp()),
        "status": l.status,
        "obtido_em": l.obtido_em.to_rfc3339(),
    });
    let _ = std::fs::create_dir_all(crate::accounts::root());
    let _ = std::fs::write(cache_path(dir), v.to_string());
}

fn header_f64(r: &ureq::http::Response<ureq::Body>, nome: &str) -> Option<f64> {
    r.headers().get(nome)?.to_str().ok()?.trim().parse().ok()
}

fn header_ts(r: &ureq::http::Response<ureq::Body>, nome: &str) -> Option<DateTime<Utc>> {
    let s: i64 = r.headers().get(nome)?.to_str().ok()?.trim().parse().ok()?;
    Utc.timestamp_opt(s, 0).single()
}

/// A menor chamada possível: Haiku, um token de saída. O que interessa são os
/// cabeçalhos; o corpo é descartado.
fn consultar(dir: &Path) -> Option<Limits> {
    let token = token(dir)?;
    let corpo = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 1,
        "system": [{"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}],
        "messages": [{"role": "user", "content": "."}]
    });

    let resp = ureq::post("https://api.anthropic.com/v1/messages")
        .header("authorization", &format!("Bearer {token}"))
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "oauth-2025-04-20,claude-code-20250219")
        .header("x-app", "cli")
        .header("user-agent", "claude-cli/2.1.276 (external, cli)")
        .header("content-type", "application/json")
        .send(corpo.to_string());

    // Token vencido ou limite estourado devolvem erro com os headers junto:
    // aproveitamos os cabeçalhos dos dois casos.
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(_)) => return None,
        Err(_) => return None,
    };

    let l = Limits {
        uso_5h: header_f64(&resp, "anthropic-ratelimit-unified-5h-utilization")?,
        uso_7d: header_f64(&resp, "anthropic-ratelimit-unified-7d-utilization")?,
        reset_5h: header_ts(&resp, "anthropic-ratelimit-unified-5h-reset"),
        reset_7d: header_ts(&resp, "anthropic-ratelimit-unified-7d-reset"),
        status: resp
            .headers()
            .get("anthropic-ratelimit-unified-status")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string(),
        obtido_em: Utc::now(),
    };
    gravar_cache(dir, &l);
    Some(l)
}

/// Cota da conta. Usa o cache enquanto ele valer; senão pergunta à API.
/// Se a consulta falhar, devolve o cache velho em vez de nada.
pub fn get(dir: &Path) -> Option<Limits> {
    if let Some(l) = ler_cache(dir, false) {
        return Some(l);
    }
    consultar(dir).or_else(|| ler_cache(dir, true))
}
