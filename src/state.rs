//! Estado que o menu mostra, comum as duas plataformas.

use crate::accounts::{self, Account, AccountInfo};
use crate::install;
use crate::limits::{self, Limits};
use crate::sessions;
use crate::usage::{self, Usage};

pub struct State {
    pub accounts: Vec<Account>,
    pub infos: Vec<AccountInfo>,
    pub sessions: Vec<usize>,
    pub active: usize,
    pub autostart: bool,
    pub ask: bool,
    /// Uso por conta; None enquanto o cálculo de fundo não terminou.
    pub usos: Vec<Option<Usage>>,
    /// Cota real das janelas, vinda dos headers da API.
    pub limites: Vec<Option<Limits>>,
}

impl State {
    pub fn load() -> Self {
        let accounts = accounts::load();
        let active_name = accounts::active_name();
        let active = active_name
            .and_then(|n| accounts.iter().position(|a| a.name == n))
            .unwrap_or(0);
        let infos = accounts.iter().map(|a| accounts::info(&a.dir)).collect();
        let sessions = accounts
            .iter()
            .map(|a| sessions::count_for(&a.dir))
            .collect();
        Self {
            accounts,
            infos,
            sessions,
            active,
            autostart: install::autostart_enabled(),
            ask: accounts::get_flag("ask_on_launch"),
            usos: Vec::new(),
            limites: Vec::new(),
        }
    }

    pub fn refresh(&mut self) {
        // Recarrega o barato, mas preserva o uso já calculado: recomputar aqui
        // faria o menu piscar a cada 5 segundos por nada.
        let antigos: Vec<(String, Option<Usage>, Option<Limits>)> = self
            .accounts
            .iter()
            .enumerate()
            .map(|(i, a)| {
                (
                    a.name.clone(),
                    self.usos.get(i).cloned().flatten(),
                    self.limites.get(i).cloned().flatten(),
                )
            })
            .collect();
        *self = Self::load();
        self.usos = self
            .accounts
            .iter()
            .map(|a| {
                antigos
                    .iter()
                    .find(|(n, ..)| n == &a.name)
                    .and_then(|(_, u, _)| u.clone())
            })
            .collect();
        self.limites = self
            .accounts
            .iter()
            .map(|a| {
                antigos
                    .iter()
                    .find(|(n, ..)| n == &a.name)
                    .and_then(|(.., l)| l.clone())
            })
            .collect();
    }

    /// Recalcula o uso de todas as contas. Custa centenas de ms: só fora do
    /// laço do menu, em thread de fundo.
    pub fn calcular_usos(
        accounts: &[accounts::Account],
    ) -> (Vec<Option<Usage>>, Vec<Option<Limits>>) {
        let mut usos = Vec::with_capacity(accounts.len());
        let mut lims = Vec::with_capacity(accounts.len());
        for a in accounts {
            let l = limits::get(&a.dir);
            // Com os resets verdadeiros em mãos, os tokens são somados
            // exatamente nas janelas a que o percentual se refere.
            let janelas = l.as_ref().and_then(|l| {
                Some((
                    l.reset_5h? - chrono::Duration::hours(5),
                    l.reset_7d? - chrono::Duration::days(7),
                ))
            });
            usos.push(Some(usage::compute(&a.dir, janelas)));
            lims.push(l);
        }
        (usos, lims)
    }

    /// Troca a conta das próximas sessões. Nada em andamento é afetado.
    pub fn switch(&mut self, i: usize) {
        if let Some(acc) = self.accounts.get(i)
            && accounts::set_active(&acc.name).is_ok()
        {
            self.active = i;
        }
    }

    /// Bloco de uso de uma conta: nome, janela de 5h e ciclo de 7 dias.
    ///
    /// Com a cota vinda da API, a barra é a cota, o mesmo número do `/usage`.
    /// Sem ela (offline, login vencido), a barra volta a ser o tempo corrido da
    /// janela, e o rótulo diz qual das duas coisas está na tela.
    pub fn linhas_uso(&self, i: usize) -> Vec<String> {
        let agora = chrono::Utc::now();
        let u = self.usos.get(i).cloned().flatten();
        let l = self.limites.get(i).cloned().flatten();
        if u.is_none() && l.is_none() {
            return Vec::new();
        }
        let mut out = vec![format!("   {}", self.accounts[i].name)];
        let eq = |t: Option<u64>| {
            t.map(|t| format!("  ·  {} eq", usage::fmt_tokens(t)))
                .unwrap_or_default()
        };

        match &l {
            Some(l) => {
                out.push(format!(
                    "     5h  {} {:>3.0}% da cota{}  ·  reseta {}",
                    usage::barra(l.uso_5h, 10),
                    l.uso_5h * 100.0,
                    eq(u.as_ref().map(|u| u.sessao_tokens)),
                    fmt_reset(l.reset_5h, agora),
                ));
                out.push(format!(
                    "     7d  {} {:>3.0}% da cota{}  ·  reseta {}",
                    usage::barra(l.uso_7d, 10),
                    l.uso_7d * 100.0,
                    eq(u.as_ref().map(|u| u.semana_tokens)),
                    fmt_reset(l.reset_7d, agora),
                ));
            }
            None => {
                let Some(u) = &u else { return out };
                match (u.sessao_fracao(agora), u.sessao_fim) {
                    (Some(f), Some(fim)) => out.push(format!(
                        "     5h  {} {:>3.0}% do tempo  ·  {} eq  ·  até {}",
                        usage::barra(f, 10),
                        f * 100.0,
                        usage::fmt_tokens(u.sessao_tokens),
                        fim.with_timezone(&chrono::Local).format("%H:%M"),
                    )),
                    _ => out.push("     5h  ░░░░░░░░░░  sem sessão em curso".to_string()),
                }
                if let Some((f, dia)) = u.semana_fracao(agora) {
                    out.push(format!(
                        "     7d  {} dia {dia}/7  ·  {} eq",
                        usage::barra(f, 10),
                        usage::fmt_tokens(u.semana_tokens),
                    ));
                }
            }
        }
        out
    }

    /// Texto do tooltip / titulo da bandeja.
    pub fn active_account(&self) -> Option<&accounts::Account> {
        self.accounts.get(self.active)
    }

    pub fn active_color(&self) -> [u8; 3] {
        match self.active_account() {
            Some(a) => crate::icon::color_for(&a.name),
            None => [0x9C, 0xA3, 0xAF],
        }
    }

    /// Linha da conta no menu e no seletor de teclado.
    pub fn entry_label(&self, i: usize) -> String {
        let acc = &self.accounts[i];
        let info = &self.infos[i];
        let mut s = acc.name.clone();
        if let Some(email) = &info.email {
            s.push_str("  ·  ");
            s.push_str(email);
        }
        if info.token_expired {
            s.push_str("  ·  login expirado");
        }
        let n = self.sessions[i];
        if n > 0 {
            s.push_str(&format!("  ·  {n} rodando"));
        }
        s
    }

    pub fn tooltip(&self) -> String {
        match self.active_account() {
            Some(acc) => {
                let info = &self.infos[self.active];
                let who = accounts::label(acc, info);
                let total: usize = self.sessions.iter().sum();
                if total > 0 {
                    format!(
                        "Claude Code: novas sessões em '{}' ({who})\n{total} sessão(ões) em andamento",
                        acc.name
                    )
                } else {
                    format!("Claude Code: novas sessões em '{}' ({who})", acc.name)
                }
            }
            None => "Claude Code: nenhuma conta registrada".to_string(),
        }
    }
}

/// "14:10" quando é hoje, "22/09 20:00" quando não é.
fn fmt_reset(
    t: Option<chrono::DateTime<chrono::Utc>>,
    agora: chrono::DateTime<chrono::Utc>,
) -> String {
    let Some(t) = t else { return "?".into() };
    let local = t.with_timezone(&chrono::Local);
    if local.date_naive() == agora.with_timezone(&chrono::Local).date_naive() {
        local.format("%H:%M").to_string()
    } else {
        local.format("%d/%m %H:%M").to_string()
    }
}
