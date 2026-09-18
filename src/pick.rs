//! Escolha da conta no nascimento da sessão, navegando pelo teclado.
//!
//! Tudo é desenhado no stderr: o stdout carrega só o diretório escolhido, para
//! o shim capturar com `$(...)`.

use crate::state::State;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use std::io::{self, Write};

/// Índice escolhido, ou `None` para manter o padrão.
pub fn choose(st: &State) -> Option<usize> {
    if st.accounts.is_empty() {
        return None;
    }
    // Sem modo cru não dá para ler setas; cai no prompt de digitar o número.
    if terminal::enable_raw_mode().is_err() {
        return fallback(st);
    }
    let r = navegar(st);
    let _ = terminal::disable_raw_mode();
    let mut err = io::stderr();
    let _ = execute!(err, cursor::Show);
    let _ = writeln!(err);
    r
}

fn desenhar(st: &State, cursor_em: usize, primeira_vez: bool) -> io::Result<()> {
    let mut err = io::stderr();
    // Contagem exata do que este desenho emite, senão sobra cabeçalho na tela:
    // título + branco (2) + uma por conta (N) + branco (1) + rodapé (1).
    let linhas = (st.accounts.len() + 4) as u16;
    if !primeira_vez {
        queue!(
            err,
            cursor::MoveToPreviousLine(linhas),
            Clear(ClearType::FromCursorDown)
        )?;
    }

    queue!(
        err,
        SetAttribute(Attribute::Bold),
        Print("  Claude Code: qual conta?"),
        SetAttribute(Attribute::Reset),
        Print("\r\n\r\n"),
    )?;

    for i in 0..st.accounts.len() {
        let aqui = i == cursor_em;
        let ativa = i == st.active;
        // A seta mostra onde o teclado está; a bolinha, qual é o padrão de hoje.
        let seta = if aqui { "›" } else { " " };
        let marca = if ativa { "●" } else { "○" };
        if aqui {
            queue!(
                err,
                SetForegroundColor(Color::Cyan),
                SetAttribute(Attribute::Bold)
            )?;
        } else {
            queue!(err, SetAttribute(Attribute::Dim))?;
        }
        queue!(
            err,
            Print(format!("  {seta} {marca} {}", st.entry_label(i))),
            ResetColor,
            SetAttribute(Attribute::Reset),
            Print("\r\n"),
        )?;
    }

    queue!(
        err,
        Print("\r\n"),
        SetAttribute(Attribute::Dim),
        Print("  ↑↓ escolher · enter confirmar · esc padrão"),
        SetAttribute(Attribute::Reset),
        Print("\r\n"),
    )?;
    err.flush()
}

fn navegar(st: &State) -> Option<usize> {
    let mut cursor_em = st.active.min(st.accounts.len() - 1);
    let mut err = io::stderr();
    let _ = execute!(err, cursor::Hide);
    if desenhar(st, cursor_em, true).is_err() {
        return None;
    }

    loop {
        let Ok(ev) = event::read() else { return None };
        let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = ev
        else {
            continue;
        };
        // Terminais que falam o protocolo estendido mandam press e release:
        // sem este filtro, cada tecla contaria duas vezes.
        if kind != event::KeyEventKind::Press {
            continue;
        }

        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                cursor_em = if cursor_em == 0 {
                    st.accounts.len() - 1
                } else {
                    cursor_em - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                cursor_em = (cursor_em + 1) % st.accounts.len();
            }
            KeyCode::Home => cursor_em = 0,
            KeyCode::End => cursor_em = st.accounts.len() - 1,
            KeyCode::Enter | KeyCode::Char(' ') => return Some(cursor_em),
            KeyCode::Esc => return None,
            KeyCode::Char('c' | 'd') if modifiers.contains(KeyModifiers::CONTROL) => return None,
            // Número escolhe e confirma de uma vez.
            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                let n = (c as usize) - ('0' as usize) - 1;
                if n < st.accounts.len() {
                    return Some(n);
                }
            }
            _ => continue,
        }
        if desenhar(st, cursor_em, false).is_err() {
            return None;
        }
    }
}

/// Terminal que não aceita modo cru: volta ao prompt de digitar o número.
fn fallback(st: &State) -> Option<usize> {
    let mut err = io::stderr();
    let _ = writeln!(err);
    let _ = writeln!(err, "  Claude Code: qual conta?");
    for i in 0..st.accounts.len() {
        let marca = if i == st.active { "●" } else { " " };
        let _ = writeln!(err, "   {marca} {}) {}", i + 1, st.entry_label(i));
    }
    let padrao = st
        .accounts
        .get(st.active)
        .map(|a| a.name.as_str())
        .unwrap_or("");
    let _ = write!(err, "\n  Enter = {padrao}   › ");
    let _ = err.flush();

    let mut linha = String::new();
    if io::stdin().read_line(&mut linha).is_err() {
        return None;
    }
    let escolha = linha.trim();
    if escolha.is_empty() {
        return None;
    }
    if let Ok(n) = escolha.parse::<usize>() {
        return n.checked_sub(1).filter(|i| *i < st.accounts.len());
    }
    st.accounts.iter().position(|a| a.name == escolha)
}
