# claude-code-switcher

Troque a conta do Claude Code por um ícone na bandeja do sistema, valendo para
tudo que abrir depois, sem derrubar nada que já esteja rodando.

```
        ┌──────────────────────────────────────────────────────┐
        │  Novas sessões usam:                                 │
        │  ● trabalho  ·  voce@empresa.com      ·  1 rodando   │
        │  ○ pessoal   ·  voce@gmail.com        ·  2 rodando   │
        ├──────────────────────────────────────────────────────┤
        │  Uso da cota                                         │
        │     trabalho                                         │
        │       5h  ██░░░░░░░░  20% da cota · reseta 14:10     │
        │       7d  ██░░░░░░░░  16% da cota · reseta 22/09     │
        │     pessoal                                          │
        │       5h  █░░░░░░░░░   5% da cota · reseta 12:50     │
        │       7d  █░░░░░░░░░   9% da cota · reseta 24/09     │
        ├──────────────────────────────────────────────────────┤
        │  Abrir sessão nesta conta                            │
        │  Entrar / trocar login desta conta...                │
        │  Perguntar a conta a cada sessão            ✓        │
        │  Iniciar com o sistema                      ✓        │
        └──────────────────────────────────────────────────────┘
```

## O problema

Se você usa mais de uma conta do Claude Code na mesma máquina, digamos uma do
trabalho e uma pessoal, trocar entre elas é incômodo:

* O `/login` troca a credencial dentro do mesmo diretório de configuração. Se
  você tem sessões em andamento naquela conta, elas sentem a troca.
* Um alias de shell (`alias claude-pessoal='CLAUDE_CONFIG_DIR=... claude'`) só
  existe no bash interativo. O Cursor, o VS Code, atalhos do menu e qualquer
  script chamam `claude` direto e caem sempre na conta padrão.
* Você acaba precisando lembrar, a cada terminal aberto, em qual conta está.

O objetivo aqui é simples: um lugar só para escolher a conta, valendo para tudo
que nascer depois, e nunca tocando no que já está rodando.

## Como funciona

O Claude Code lê a variável `CLAUDE_CONFIG_DIR` **uma única vez, quando o
processo sobe**, e cada conta guarda credenciais, sessões e configurações no
próprio diretório. Disso saem as duas garantias do projeto:

1. Trocar de conta é só reescrever um arquivo de estado. Processos vivos seguem
   no diretório que capturaram ao nascer.
2. O refresh de token de uma conta nunca toca o arquivo da outra, porque são
   arquivos diferentes.

A troca passa a valer em todo lugar por causa de um **shim**: um `claude` que
entra na frente do PATH, resolve a conta ativa no instante do lançamento e então
executa o Claude Code de verdade.

```
    você troca a conta            ~/.claude-accounts/active
    (ícone ou terminal)  ───────►  contém: "pessoal"
                                          │
    você digita `claude`                  │ lê no instante do lançamento
    no terminal ou no Cursor ──►  shim ───┘
                                    │
                                    └─► exec claude, com CLAUDE_CONFIG_DIR certo

    sessões que já estavam rodando: intocadas
```

## Instalação

### Antes de começar

**Linux com um indicador de bandeja.** No Ubuntu com GNOME a extensão
`ubuntu-appindicators` já vem habilitada. Em outros ambientes, instale o suporte
a AppIndicator do seu desktop.

**Claude Code instalado.** Se ainda não tem, veja
[claude.com/claude-code](https://claude.com/claude-code).

**Rust**, apenas se você for compilar do código. Quem instala pelo binário
pronto não precisa. A forma oficial é:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Confira com `cargo --version`. Também dá para instalar pelo gerenciador de
pacotes (`apt install cargo`, `dnf install cargo`), mas versões de distribuição
costumam ficar para trás. É preciso Rust 1.85 ou mais novo.

### Instalar

A forma mais curta, com o binário já compilado:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/diazmateus/claude-code-switcher/main/install.sh | sh
```

O script baixa o binário da release mais recente, confere o checksum e coloca em
`~/.local/bin`. Não usa `sudo` e não escreve fora do seu diretório pessoal. Se
preferir ler antes de executar, e é uma boa prática, abra o
[install.sh](install.sh).

Se você já tem Rust e prefere compilar, dá para instalar direto do repositório
sem clonar nada:

```sh
cargo install --git https://github.com/diazmateus/claude-code-switcher
```

Ou clonando, se quiser mexer no código:

```sh
git clone https://github.com/diazmateus/claude-code-switcher
cd claude-code-switcher
cargo build --release
install -m 755 target/release/ccswitch ~/.local/bin/ccswitch
```

### Registrar suas contas

**Se você já usa mais de uma conta**, provavelmente já tem os diretórios
criados, por `CLAUDE_CONFIG_DIR` na mão ou por algum alias. O ccswitch encontra
todos sozinho:

```sh
ccswitch detect
```

```
Contas encontradas

  + trabalho     voce@empresa.com             (registrada agora)
      /home/voce/.claude
  + pessoal      voce@gmail.com               (registrada agora)
      /home/voce/.claude-pessoal
  ! antiga       voce@gmail.com               (mesma conta que 'pessoal', ignorada)
      /home/voce/.claude-antiga
```

Ele varre o seu diretório pessoal atrás de `.claude` e `.claude-*`, lê o email
de cada um e deixa explícito o que achou. Diretórios que apontam para a **mesma
conta** aparecem marcados com `!` e são ignorados, em vez de virarem duas
entradas que fariam você escolher entre opções idênticas.

O `ccswitch install` roda essa mesma detecção sozinho quando ainda não há
nenhuma conta registrada, então na maioria dos casos você não precisa chamar o
`detect` à mão.

**Se você ainda não tem uma segunda conta**, crie o diretório e entre nela:

```sh
ccswitch add pessoal        # cria ~/.claude-pessoal e registra
ccswitch login pessoal      # abre o /login do Claude Code naquela conta
```

O `login` abre o Claude Code apontando para o diretório daquela conta, faz o
fluxo normal de autenticação e grava a credencial lá dentro. Sua outra conta não
é tocada, porque cada uma tem o próprio arquivo de credencial. Dá para fazer o
mesmo pelo menu da bandeja, em "Entrar / trocar login desta conta".

Para registrar um diretório que está fora do padrão:

```sh
ccswitch add trabalho ~/algum/outro/caminho
```

### Instalar os atalhos

```sh
ccswitch install
```

Isso escreve o shim, coloca o diretório dele na frente do PATH e configura o
início automático junto com o sistema.

**Abra um terminal novo depois disso.** Programas já abertos, como o Cursor,
só enxergam o PATH novo depois de reiniciar. Se algo não pegar, rode
`ccswitch doctor`, que mostra onde o Claude Code foi encontrado e se o shim está
mesmo na frente do PATH.

## Comandos

| Comando | O que faz |
| --- | --- |
| `ccswitch` | Abre o ícone na bandeja |
| `ccswitch use <conta>` | Troca a conta das próximas sessões |
| `ccswitch <conta>` | Forma curta do anterior |
| `ccswitch run <conta> ...` | Sessão pontual em outra conta, sem trocar o padrão |
| `ccswitch list` | Contas, sessões rodando e estado da instalação |
| `ccswitch usage` | Cota das janelas de 5 horas e 7 dias, por conta |
| `ccswitch ask on\|off` | Liga ou desliga a pergunta a cada sessão nova |
| `ccswitch detect` | Procura contas que já existem e registra |
| `ccswitch add <nome> [dir]` | Registra uma conta |
| `ccswitch login <conta>` | Entra ou troca o login de uma conta |
| `ccswitch install` | Instala shim, PATH e início automático |
| `ccswitch doctor` | Diagnóstico de onde está o Claude e se o shim pega |

## Perguntar a conta a cada sessão

Com `ccswitch ask on`, ou pelo item correspondente no menu, toda sessão nova
pergunta em qual conta deve nascer:

```
  Claude Code: qual conta?
    ● trabalho  ·  voce@empresa.com  ·  1 rodando
  › ○ pessoal   ·  voce@gmail.com    ·  2 rodando
  ↑↓ escolher · enter confirmar · esc padrão
```

A seta mostra onde está o teclado e a bolinha cheia mostra qual é o padrão de
hoje. Navegue com as setas (ou `j`/`k`, ou Tab), confirme com Enter ou espaço,
cancele com Esc ou Ctrl+C. Digitar o número escolhe e confirma de uma vez.

Ele só pergunta quando existe terminal de verdade e a sessão é interativa.
Segue o padrão em silêncio quando a saída é pipe ou script, quando os argumentos
incluem `-p`, `--print`, `-c`, `--continue`, `-r` ou `--resume`, ou quando
`CLAUDE_CONFIG_DIR` já vem definido no ambiente. Isso evita travar automação
esperando uma resposta que ninguém vai digitar. Em terminais que não aceitam
modo cru, ele cai sozinho num prompt de digitar o número.

## Uso da cota

O menu mostra, por conta, o mesmo percentual que o `/usage` exibe dentro do
Claude Code:

```
Uso da cota

   trabalho
     5h  ██░░░░░░░░  20% da cota  ·  4,2M eq   ·  reseta 14:10
     7d  ██░░░░░░░░  16% da cota  ·  51,1M eq  ·  reseta 22/09 20:00

   pessoal
     5h  █░░░░░░░░░   5% da cota  ·  22,4M eq  ·  reseta 12:50
     7d  █░░░░░░░░░   9% da cota  ·  74,2M eq  ·  reseta 24/09 19:00
```

**De onde vem o percentual.** Esse número não é gravado em lugar nenhum: ele
chega nos headers `anthropic-ratelimit-unified-*` das respostas da API. Para
lê-lo fora de uma sessão, o ccswitch manda a menor requisição possível (Haiku,
`max_tokens: 1`, cerca de 22 tokens de entrada) com o token da própria conta, lê
os cabeçalhos e descarta a resposta. Os horários de reset vêm daí também, então
as janelas são as verdadeiras e não uma estimativa.

O resultado fica em cache por 15 minutos, então abrir o menu várias vezes não
vira várias chamadas. São cerca de 4 consultas por hora por conta. Se a consulta
falhar, por falta de rede ou login vencido, vale o último valor conhecido. Não
havendo nenhum, a barra passa a mostrar o tempo corrido da janela e o rótulo
muda de "da cota" para "do tempo", para nunca passar um número por outro.

**Os "eq" ao lado.** São tokens equivalentes, lidos dos transcripts locais da
própria conta, para dar a magnitude do consumo. Cada tipo entra pelo que pesa e
não pelo volume bruto: cache lido custa 10% do input e domina o volume, então
somá-lo inteiro inflava o número em cerca de 6 vezes. Modelos também são
ponderados, com Haiku pesando cerca de 1/15 de Opus.

## Por dentro

```
~/.claude-accounts/
├── accounts.conf          nome=diretório de cada conta
├── active                 conta usada por sessões novas
├── config                 opções (ask_on_launch)
├── cota-<conta>.json      cota lida da API, válida por 15 minutos
├── ciclo-<conta>          âncora do ciclo de 7 dias (só sem acesso à API)
└── bin/claude             o shim
```

O shim é um script de shell de poucas linhas. Ele resolve a conta, encontra o
binário real do Claude Code e faz `exec`, preservando o PID e os argumentos. Um
detalhe importante: ele carrega uma marca no cabeçalho e descarta qualquer
candidato que a contenha, de modo que um shim nunca chame outro shim. Sem isso,
duas instalações no PATH entram em laço infinito.

A instalação escreve o PATH em três lugares, porque cada um alcança um público
diferente:

* `~/.bashrc`, para shells interativos.
* O arquivo de login que o bash realmente lê, que é o primeiro entre
  `.bash_profile`, `.bash_login` e `.profile`. Escrever no `.profile` quando
  existe um `.bash_profile` não tem efeito nenhum, porque o bash lê só o
  primeiro que encontra.
* `~/.config/environment.d/50-claude-accounts.conf`, para aplicativos gráficos,
  que não leem `.bashrc`.

O ícone é desenhado em código, sem arquivo de imagem, e ganha uma cor por conta
derivada do nome. Assim dá para saber de relance em qual conta a próxima sessão
vai nascer.

A bandeja usa o protocolo StatusNotifierItem via D-Bus, sem GTK. O cálculo de
uso custa algumas centenas de milissegundos, então roda numa thread de fundo e o
menu apenas recolhe o resultado pronto, sem nunca travar.

## Limitações conhecidas

* **Só Linux.** Houve uma versão para Windows, removida. Lá o PATH alcança
  apenas programas que procuram o `claude` por conta própria, e integrações que
  chamam o binário por caminho absoluto continuavam na conta antiga.
* **A troca só vale para sessões novas.** Isso é intencional e é o ponto do
  projeto, mas vale dizer: não existe trocar a conta de uma sessão em andamento
  sem derrubá-la.
* **Programas já abertos precisam ser reiniciados** para enxergar o PATH novo.
* **O percentual de cota custa uma requisição mínima.** Se você prefere não
  fazer nenhuma chamada, apague o arquivo de cache e a barra volta a mostrar o
  tempo corrido da janela.
* **As credenciais ficam onde o Claude Code as põe.** Este projeto não lê, não
  copia e não move nenhum token, exceto para ler o `accessToken` da própria
  conta no momento de consultar a cota.

## Requisitos

* Linux com um indicador de bandeja. No GNOME, a extensão AppIndicator.
* Rust 1.85 ou mais novo para compilar.
* Claude Code instalado.

## Licença

MIT. Veja [LICENSE](LICENSE).

Este é um projeto independente, sem vínculo com a Anthropic.
