mod accounts;
mod icon;
mod install;
mod launch;
mod limits;
mod pick;
mod real;
mod usage;
mod sessions;
mod state;

mod tray_linux;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("tray");

    match cmd {
        "tray" => run_tray(),

        // Pergunta a conta no nascimento da sessao. O menu vai para stderr e
        // so' o diretorio escolhido sai no stdout, para o shim capturar.
        "pick" => {
            use std::io::IsTerminal;
            let st = state::State::load();
            if st.accounts.is_empty() {
                return ExitCode::FAILURE;
            }
            let padrao = st
                .active_account()
                .map(|a| a.dir.display().to_string())
                .unwrap_or_default();

            // Sem terminal (script, pipe, CI) nunca pergunta: usa o padrao.
            if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
                println!("{padrao}");
                return ExitCode::SUCCESS;
            }

            let dir = match pick::choose(&st) {
                Some(i) => st.accounts[i].dir.display().to_string(),
                None => padrao,
            };
            println!("{dir}");
            ExitCode::SUCCESS
        }

        // Usado pelos shims: caminho do Claude Code de verdade.
        "real-claude" => match real::encontrar() {
            Some(p) => {
                println!("{}", p.display());
                ExitCode::SUCCESS
            }
            None => ExitCode::FAILURE,
        },

        // Diagnóstico: por que a troca não está pegando.
        "doctor" => {
            println!("Claude Code");
            match real::encontrar() {
                Some(p) => println!("  usando: {}", p.display()),
                None => println!("  NÃO ENCONTRADO, o shim não tem o que chamar"),
            }
            println!("\n  lugares olhados:");
            for (p, estado) in real::diagnostico() {
                println!("    [{estado}] {}", p.display());
            }

            println!("\nPATH");
            let bin = accounts::bin_dir();
            println!("  shim instalado em: {}", bin.display());
            println!("  shim na frente do PATH: {}", yes_no(install::shim_wins_path()));
            let nome = "claude";
            for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
                if dir.join(nome).exists() {
                    let marca = if dir == bin { "  <- nosso shim" } else { "" };
                    println!("    {}{marca}", dir.display());
                }
            }

            println!("\nContas");
            println!("  ativa: {}", accounts::active_name().unwrap_or_else(|| "nenhuma".into()));
            match std::env::var("CLAUDE_CONFIG_DIR") {
                Ok(v) => println!("  CLAUDE_CONFIG_DIR já definido neste processo: {v}"),
                Err(_) => println!("  CLAUDE_CONFIG_DIR não definido (o shim vai resolver)"),
            }
            println!("  perguntar a cada sessão: {}", yes_no(accounts::get_flag("ask_on_launch")));
            ExitCode::SUCCESS
        }

        // Liga/desliga a pergunta a cada nova sessao.
        "ask" => {
            let on = match args.get(1).map(String::as_str) {
                Some("on") | Some("sim") => true,
                Some("off") | Some("nao") | Some("não") => false,
                _ => {
                    println!(
                        "perguntar a conta a cada sessão: {}",
                        if accounts::get_flag("ask_on_launch") { "ligado" } else { "desligado" }
                    );
                    println!("uso: ccswitch ask on | off");
                    return ExitCode::SUCCESS;
                }
            };
            match accounts::set_flag("ask_on_launch", on) {
                Ok(()) => {
                    println!("perguntar a cada sessão: {}", if on { "ligado" } else { "desligado" });
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("erro: {e}");
                    ExitCode::FAILURE
                }
            }
        }

        // Usado pelos shims: imprime o config dir da conta ativa e mais nada.
        "resolve" => {
            match accounts::active_name().and_then(|n| accounts::find(&n)) {
                Some(acc) if acc.dir.is_dir() => {
                    println!("{}", acc.dir.display());
                    ExitCode::SUCCESS
                }
                _ => ExitCode::FAILURE,
            }
        }

        "use" | "switch" => match args.get(1) {
            Some(name) => match accounts::set_active(name) {
                Ok(()) => {
                    println!("novas sessões usam '{name}'");
                    println!("as que já estão rodando seguem intactas");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("erro: {e}");
                    ExitCode::FAILURE
                }
            },
            None => {
                eprintln!("uso: ccswitch use <nome>");
                ExitCode::FAILURE
            }
        },

        // Sessao pontual em outra conta, sem mexer no padrao global.
        "run" => {
            let Some(name) = args.get(1) else {
                eprintln!("uso: ccswitch run <conta> [argumentos do claude]");
                return ExitCode::FAILURE;
            };
            let Some(acc) = accounts::find(name) else {
                eprintln!("erro: conta '{name}' nao registrada");
                return ExitCode::FAILURE;
            };
            let rest: Vec<&String> = args.iter().skip(2).collect();
            let mut cmd = std::process::Command::new("claude");
            cmd.args(&rest)
                .env("CLAUDE_CONFIG_DIR", &acc.dir)
                .env("CLAUDE_ACCOUNT", &acc.name);
            match cmd.status() {
                Ok(st) => ExitCode::from(st.code().unwrap_or(0) as u8),
                Err(e) => {
                    eprintln!("erro ao iniciar o claude: {e}");
                    ExitCode::FAILURE
                }
            }
        }

        // Uso por conta, lido dos transcripts da propria conta.
        "usage" | "uso" => {
            let mut st = state::State::load();
            let (usos, limites) = state::State::calcular_usos(&st.accounts);
            st.usos = usos;
            st.limites = limites;
            println!("Uso da cota\n");
            for i in 0..st.accounts.len() {
                for linha in st.linhas_uso(i) {
                    println!("{linha}");
                }
                println!();
            }
            ExitCode::SUCCESS
        }

        "list" | "ls" | "status" => {
            let st = state::State::load();
            if st.accounts.is_empty() {
                println!("nenhuma conta registrada em {}", accounts::conf_path().display());
                return ExitCode::FAILURE;
            }
            for i in 0..st.accounts.len() {
                let mark = if i == st.active { "●" } else { "○" };
                println!("{mark} {}", st.entry_label(i));
                println!("    {}", st.accounts[i].dir.display());
            }
            println!();
            println!("shim instalado: {}", yes_no(install::shim_installed()));
            println!("shim na frente do PATH: {}", yes_no(install::shim_wins_path()));
            println!("inicia com o sistema: {}", yes_no(install::autostart_enabled()));
            ExitCode::SUCCESS
        }

        "add" => {
            let Some(name) = args.get(1) else {
                eprintln!("uso: ccswitch add <nome> [dir]");
                return ExitCode::FAILURE;
            };
            let dir = args
                .get(2)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| accounts::home().join(format!(".claude-{name}")));
            match accounts::add(name, &dir) {
                Ok(()) => {
                    println!("conta '{name}' registrada em {}", dir.display());
                    println!("agora entre nela pelo menu da bandeja, ou:");
                    println!("  CLAUDE_CONFIG_DIR={} claude /login", dir.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("erro: {e}");
                    ExitCode::FAILURE
                }
            }
        }

        "install" => {
            let autostart = !args.iter().any(|a| a == "--no-autostart");
            match install::install_all(autostart) {
                Ok(r) => {
                    for line in r.0 {
                        println!("· {line}");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("erro: {e}");
                    ExitCode::FAILURE
                }
            }
        }

        // Diagnostico: escreve o icone em RGBA cru, para inspecao.
        "icon-dump" => {
            let size: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(64);
            let name = args.get(2).cloned().unwrap_or_else(|| "trabalho".into());
            let data = icon::rgba(size, icon::color_for(&name));
            match std::fs::write(format!("icon-{name}-{size}.rgba"), &data) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => { eprintln!("erro: {e}"); ExitCode::FAILURE }
            }
        }

        "help" | "--help" | "-h" => {
            help();
            ExitCode::SUCCESS
        }

        other => {
            // Atalho: "ccswitch pessoal" troca para a conta 'pessoal'.
            if accounts::find(other).is_some() {
                return match accounts::set_active(other) {
                    Ok(()) => {
                        println!("novas sessões usam '{other}'");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("erro: {e}");
                        ExitCode::FAILURE
                    }
                };
            }
            eprintln!("comando desconhecido: {other}");
            help();
            ExitCode::FAILURE
        }
    }
}

fn yes_no(b: bool) -> &'static str {
    if b { "sim" } else { "não" }
}

fn run_tray() -> ExitCode {
    match tray_linux::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::FAILURE
        }
    }
}

fn help() {
    println!(
        "claude-switcher: troca a conta do Claude Code sem derrubar sessão

  ccswitch                  abre o ícone na bandeja
  ccswitch use <nome>       troca a conta das próximas sessões
  ccswitch <nome>           idem, forma curta
  ccswitch list             contas, sessões rodando e estado da instalação
  ccswitch run <conta> ...   sessao pontual em outra conta, sem trocar o padrao
  ccswitch ask on|off       perguntar a conta a cada sessao nova
  ccswitch add <nome> [dir] registra uma conta nova
  ccswitch install          instala os atalhos no PATH e o início automático
  ccswitch doctor           diagnóstico: onde está o Claude e se o shim pega
  ccswitch resolve          imprime o dir da conta ativa (usado pelos shims)

A troca vale só para sessões novas: o Claude Code lê CLAUDE_CONFIG_DIR uma
única vez, ao subir. O que já está rodando continua na conta em que nasceu."
    );
}
