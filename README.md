# MoonProfile

Streaming Moonlight/Apollo do Steam Deck com perfis por contexto (docked
vs handheld), sem precisar reconfigurar bitrate/resolução/HDR na mão a
cada sessão.

Dois componentes:

- **`moon_profile_decky/`** - plugin [Decky Loader](https://decky.xyz/)
  que roda no Steam Deck.
- **`moon_profile_runner/`** - daemon que roda no PC host (onde o Apollo
  está instalado).

## Requisitos

- **Host** (PC que vai ser streamado):
  - [Apollo](https://github.com/ClassicOldSong/Apollo) 0.4.8+ configurado
    e rodando.
  - Linux com KDE Plasma 6 em Wayland.
  - GPU com suporte a encode via VAAPI (testado com AMD RDNA).
  - `ydotool` + `ydotoold` instalados, com
    `systemctl --user enable --now ydotool.service` - só necessário se
    for usar a opção "mover cursor pro canto" de algum perfil.
- **Steam Deck** (ou qualquer cliente com Decky Loader):
  - [Decky Loader](https://wiki.deckbrew.xyz/en/plugin-dev/getting-started)
    instalado.
  - Cliente [Moonlight Flatpak](https://flathub.org/apps/com.moonlight_stream.Moonlight)
    (`com.moonlight_stream.Moonlight`) instalado.

## Instalação

### 1. Runner (no host)

Via AUR:

```bash
yay -S moon-profile-runner-git
```

Isso já builda, instala e registra o autostart de sessão gráfica. Sem
`yay`/AUR, dá pra buildar manualmente:

```bash
git clone https://github.com/egionCode/moon_profile.git
cd moon_profile/moon_profile_runner
./install.sh
```

### 2. Plugin (no Steam Deck)

Baixe o zip da [última release](https://github.com/egionCode/moon_profile/releases/latest)
(`moonprofile-decky-*.zip`) e extraia em
`/home/deck/homebrew/plugins/`, depois reinicie o Decky Loader:

```bash
ssh deck@STEAMDECK_IP "systemctl --user restart plugin_loader"
```

Ative o plugin "MoonProfile" no menu do Decky Loader (ícone de satélite
no Quick Access).

### 3. Configuração inicial

Na aba "Configurações" do plugin (Quick Access → MoonProfile → engrenagem):

1. **Config do Apollo**: IP do host, usuário e senha de admin do Apollo.
2. **Runner**: porta do Runner (padrão `47991`).
3. **Perfis**: edite os perfis de exemplo ou crie os seus - resolução,
   fps, bitrate, codec e HDR do lado Moonlight; monitor alvo e monitores
   a desligar do lado host.
4. No Quick Access, clique em "Sincronizar jogos do host" - isso cria um
   atalho na biblioteca pra cada jogo Steam instalado no host, com
   capa/hero baixados automaticamente.

Pra jogar, clique "Jogar" normalmente num desses atalhos.

## Como funciona

```
[Deck: clica "Jogar" num atalho sincronizado]
    ↓
Detecta contexto (docked/handheld) e escolhe o perfil correspondente
    ↓
Configura o Apollo e avisa o Runner, que liga a tela do host
(monitor certo, resolução, HDR)
    ↓
Cliente Moonlight conecta e o stream sobe
    ↓
[jogo fecha - sozinho ou via "Fechar conexão" no Quick Access]
    ↓
Runner percebe que o jogo encerrou, avisa o Apollo (Deck desconecta na
hora) e restaura a tela do host
```

## Licença

GPL-3.0 - ver [`LICENSE`](LICENSE).
