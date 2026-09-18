//! Bandeja no Linux, via StatusNotifierItem (D-Bus puro, sem GTK).
//! No GNOME depende da extensao AppIndicator, que o Ubuntu ja' habilita.

use crate::state::State;
use crate::{accounts, icon, install, launch};
use ksni::blocking::TrayMethods;
use ksni::menu::{CheckmarkItem, RadioGroup, RadioItem, StandardItem};
use ksni::{Icon, MenuItem, ToolTip};

pub struct Switcher {
    pub st: State,
}

impl ksni::Tray for Switcher {
    fn id(&self) -> String {
        "claude-switcher".into()
    }

    fn title(&self) -> String {
        match self.st.active_account() {
            Some(a) => format!("Claude Code: {}", a.name),
            None => "Claude Code".into(),
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let color = self.st.active_color();
        [22u32, 32, 48]
            .iter()
            .map(|s| Icon {
                width: *s as i32,
                height: *s as i32,
                data: icon::argb(*s, color),
            })
            .collect()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Claude Code".into(),
            description: self.st.tooltip(),
            ..Default::default()
        }
    }

    // Reabrir o menu sempre relê o estado: sessões e logins mudam por fora.
    fn menu_about_to_show(&mut self) {
        self.st.refresh();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = Vec::new();

        if self.st.accounts.is_empty() {
            items.push(
                StandardItem {
                    label: "nenhuma conta registrada".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        } else {
            items.push(
                StandardItem {
                    label: "Novas sessões usam:".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            let options: Vec<RadioItem> = (0..self.st.accounts.len())
                .map(|i| RadioItem {
                    label: self.st.entry_label(i),
                    ..Default::default()
                })
                .collect();
            items.push(
                RadioGroup {
                    selected: self.st.active,
                    select: Box::new(|this: &mut Self, i| {
                        this.st.switch(i);
                    }),
                    options,
                }
                .into(),
            );
        }

        // Uso de cada conta, lido dos transcripts da própria conta.
        if self.st.usos.iter().any(Option::is_some) {
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: "Uso da cota".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            for i in 0..self.st.accounts.len() {
                for linha in self.st.linhas_uso(i) {
                    items.push(
                        StandardItem {
                            label: linha,
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                    );
                }
            }
        }

        items.push(MenuItem::Separator);

        let has_active = self.st.active_account().is_some();
        items.push(
            StandardItem {
                label: "Abrir sessão nesta conta".into(),
                enabled: has_active,
                activate: Box::new(|this: &mut Self| {
                    if let Some(acc) = this.st.active_account() {
                        launch::open_session(acc, &[]);
                    }
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Entrar / trocar login desta conta…".into(),
                enabled: has_active,
                activate: Box::new(|this: &mut Self| {
                    if let Some(acc) = this.st.active_account() {
                        launch::open_session(acc, &["/login"]);
                    }
                }),
                ..Default::default()
            }
            .into(),
        );

        items.push(MenuItem::Separator);
        items.push(
            CheckmarkItem {
                label: "Perguntar a conta a cada sessão".into(),
                checked: self.st.ask,
                activate: Box::new(|this: &mut Self| {
                    let novo = !this.st.ask;
                    if accounts::set_flag("ask_on_launch", novo).is_ok() {
                        this.st.ask = novo;
                    }
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            CheckmarkItem {
                label: "Iniciar com o sistema".into(),
                checked: self.st.autostart,
                activate: Box::new(|this: &mut Self| {
                    let mut r = install::Report(Vec::new());
                    let _ = install::set_autostart(!this.st.autostart, &mut r);
                    this.st.autostart = !this.st.autostart;
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Reinstalar atalhos no PATH".into(),
                activate: Box::new(|_this: &mut Self| {
                    let _ = install::install_all(false);
                }),
                ..Default::default()
            }
            .into(),
        );

        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Sair".into(),
                activate: Box::new(|_this: &mut Self| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

pub fn run() -> Result<(), String> {
    if accounts::load().is_empty() {
        return Err("nenhuma conta em accounts.conf".into());
    }
    let tray = Switcher { st: State::load() };
    let handle = tray
        .spawn()
        .map_err(|e| format!("bandeja indisponível: {e}"))?;

    // Uso é caro (centenas de ms): calcula fora e injeta pronto, para nunca
    // segurar o laço que atende o menu.
    {
        let h = handle.clone();
        std::thread::spawn(move || {
            loop {
                let contas = accounts::load();
                let (usos, limites) = State::calcular_usos(&contas);
                let atualizar = move |t: &mut Switcher| {
                    t.st.usos = usos.clone();
                    t.st.limites = limites.clone();
                };
                if h.update(atualizar).is_none() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        });
    }

    // Mantém o ícone e o tooltip em dia mesmo sem ninguém abrir o menu.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        // update() devolve None quando o serviço da bandeja já encerrou.
        if handle.update(|t: &mut Switcher| t.st.refresh()).is_none() {
            return Ok(());
        }
    }
}
