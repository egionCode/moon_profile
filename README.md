# MoonProfile

Streaming Moonlight/Apollo do Steam Deck com perfis por contexto (docked
vs handheld), sem precisar reconfigurar bitrate/resolução/HDR na mão amais reflete a realidade sozinho
cada sessão e sem depender de um daemon como o Buddy do MoonDeck.

O projeto é dividido em dois componentes que se falam por HTTP na rede
local:

- **`moon_profile_decky/`** - plugin [Decky Loader](https://decky.xyz/)
  que roda no Steam Deck. Detecta o contexto (docked/handheld), aplica o
  perfil certo, cria os atalhos de jogo e fala com o Runner.
- **`moon_profile_runner/`** - daemon Tauri/Rust que roda no PC host
  (onde o Apollo está instalado). Controla tudo que mexe no sistema
  operacional do host: telas/monitores, cursor, processos, ciclo de vida
  da sessão de streaming.

## Requisitos

- **Host** (PC que vai ser streamado):
  - [Apollo](https://github.com/ClassicOldSong/Apollo) 0.4.8+ configurado
    e rodando.
  - Linux com KDE Plasma 6 em Wayland (o Runner usa `kscreen-doctor` pra
    controlar monitores e `ydotool` pra mover o cursor - ambos
    específicos desse ambiente).
  - GPU com suporte a encode via VAAPI (testado com AMD RDNA).
  - `ydotool` + `ydotoold` instalados e o serviço
    `ydotool.service` habilitado (`systemctl --user enable --now
    ydotool.service`) - só necessário se for usar a opção "mover cursor
    pro canto" de algum perfil.
- **Steam Deck** (ou qualquer cliente com Decky Loader):
  - [Decky Loader](https://wiki.deckbrew.xyz/en/plugin-dev/getting-started)
    instalado.
  - Cliente [Moonlight Flatpak](https://flathub.org/apps/com.moonlight_stream.Moonlight)
    (`com.moonlight_stream.Moonlight`) instalado.

## Instalação

### 1. Runner (no host)

Ainda não publicado no AUR (planejado - `moon_profile_runner/packaging/PKGBUILD`
já existe e foi testado, falta só a publicação). Por enquanto, build manual:

```bash
git clone https://github.com/egionCode/moon_profile.git
cd moon_profile/moon_profile_runner
./install.sh
```

O `install.sh` builda o binário em modo release (precisa de Rust
instalado - `rustup` ou o pacote `rust` da distro), copia pra
`~/.local/bin/moon_profile_runner` e registra autostart de sessão gráfica
em `~/.config/autostart/` (não é serviço systemd - o app tem tray icon e
precisa de sessão gráfica ativa pra aparecer). Ele mesmo diz quando
terminar; pra rodar sem esperar o próximo login, executa o binário
instalado direto.

O Runner sobe um servidor HTTP na porta `47991` da rede local, sem
autenticação (decisão deliberada - numa LAN doméstica já confiável, o
atrito de token não compensa o ganho).

### 2. Plugin (no Steam Deck)

Não está publicado na Decky Plugin Store. Copia o diretório
`moon_profile_decky/` pra pasta de plugins do Decky Loader:

```bash
rsync -avz --delete \
    moon_profile_decky/ deck@STEAMDECK_IP:/home/deck/homebrew/plugins/moonprofile/ \
    --exclude node_modules --exclude .git --exclude .venv

ssh deck@STEAMDECK_IP "systemctl --user restart plugin_loader"
```

Se for buildar a partir do código-fonte (em vez de copiar um `dist/` já
pronto), antes do rsync:

```bash
cd moon_profile_decky
pnpm install
pnpm build
```

Depois de instalado, ative o plugin "MoonProfile" no menu do Decky
Loader (ícone de satélite no Quick Access).

### 3. Configuração inicial

Na aba "Configurações" do plugin (Quick Access → MoonProfile → engrenagem):

1. **Config do Apollo**: IP do host, usuário e senha de admin do Apollo.
2. **Runner**: porta do Runner (padrão `47991`) - o host é o mesmo
   configurado acima, Runner e Apollo sempre rodam na mesma máquina.
3. **Perfis**: edite os dois perfis de exemplo (`docked-tv-4k-hdr` e
   `handheld-native`) ou crie os seus - resolução/fps/bitrate/codec/HDR do
   lado Moonlight, e output/resolução/HDR/monitores a desligar do lado
   host (a lista de monitores é buscada ao vivo no Runner via
   `GET /displays`, não precisa digitar o nome do output na mão).
4. No Quick Access, clique em "Sincronizar jogos do host" - isso cria um
   atalho visível na biblioteca pra cada jogo Steam instalado no host,
   com capa/hero baixados automaticamente.

Pra jogar: clique "Jogar" normalmente num desses atalhos, como faria com
qualquer jogo da biblioteca.

## Como funciona

```
[Deck: clica "Jogar" num atalho sincronizado]
    │
    ▼
[runner.py - non-Steam shortcut, executado pela própria Steam]
    ├─ detecta contexto (docked/handheld) via /sys/class/drm
    ├─ escolhe o perfil correspondente
    ├─ configura o app "SteamGame" no Apollo (login + cmd, sem prep-cmd)
    ├─ registra a sessão no MoonProfile Runner (POST /session/register)
    │    └─ Runner liga a tela do host AGORA, de forma síncrona
    │       (kscreen-doctor: ativa o output, seta resolução/HDR,
    │       desliga os outros; opcionalmente abre Big Picture e/ou
    │       manda o cursor pro canto - via ydotool)
    └─ dá exec no cliente Moonlight (stream sobe com a tela já certa)
         │
         ▼
    [stream rodando]
         │
         ▼
[jogo fecha - sozinho ou via "Fechar conexão" no Quick Access]
    │
    ▼
[MoonProfile Runner]
    ├─ um watchdog em background (a cada 5s) detecta pelo processo real
    │  do SO que o jogo morreu (não confia no Apollo: o "current_app"
    │  do Apollo entra em modo "placebo" ~5s depois do lançamento e
    │  nunca mais reflete a realidade sozinho - por isso o Runner existe)
    ├─ avisa o Apollo PRIMEIRO (POST /api/apps/close) - isso derruba a
    │  conexão/stream no Deck NA HORA, sem esperar nada
    └─ SÓ DEPOIS, em background: mata o jogo se ainda estiver vivo
       (SIGTERM, espera adaptativa, SIGKILL) e restaura a tela do host
       (religa os outputs desligados, fecha o Big Picture se abriu)
```

## Licença

GPL-3.0 - ver [`LICENSE`](LICENSE).
