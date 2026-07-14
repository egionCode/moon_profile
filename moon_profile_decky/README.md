# MoonProfile

Plugin [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader) para Steam Deck que gerencia perfis de streaming [Moonlight](https://moonlight-stream.org/)/[Apollo](https://github.com/ClassicOldSong/Apollo) com detecção automática de contexto (dockado vs portátil) e configuração dinâmica do host via API REST do Apollo.

## O problema que isso resolve

Moonlight não sabe se o Deck está dockado numa TV 4K ou portátil na tela interna - dispara sempre a mesma resolução/bitrate, e o prep-cmd do Apollo é fixo, não se adapta a cenários diferentes (HDR na TV vs SDR portátil). Configurar isso manualmente toda sessão é insustentável.

O MoonProfile detecta o contexto automaticamente (via `/sys/class/drm`, checando se algum display externo está conectado) e aplica um perfil que controla, ao mesmo tempo, a configuração do cliente Moonlight (resolução, fps, bitrate, codec, HDR) e a configuração de displays do host via Apollo (output ativo, resolução, HDR/WCG, quais outputs desligar).

## Diferencial em relação ao MoonDeck

- Zero componente adicional no host - fala só com a API REST que o Apollo já expõe, sem precisar instalar/manter um daemon companion (Buddy) rodando no PC.
- Perfis de streaming editáveis direto no Deck, com detecção automática de contexto (dockado/portátil) - o MoonDeck não tem isso.
- Cada perfil controla cliente Moonlight e host simultaneamente.

## Requisitos

- Apollo 0.4.8+ rodando no host.
- KDE Plasma 6 (Wayland) no host - o controle de display usa `kscreen-doctor`.
- GPU AMD RDNA 4 ou compatível (VAAPI).
- Moonlight Flatpak (`com.moonlight_stream.Moonlight`) instalado no Deck.

## Como usar

1. **Configure o Apollo** (Quick Access → ⚙️ no título → aba "Config do Apollo"): host, usuário e senha - as mesmas credenciais do painel web do Apollo.
2. **Crie perfis** (aba "Perfis"): cada perfil tem um gatilho (`docked`, `handheld` ou `manual`), a configuração do cliente Moonlight e a configuração de displays do host. Pelo menos um perfil com gatilho `docked` e um com `handheld` cobre a detecção automática.
3. **Ajuste a posição do botão** (aba "Posição do botão", opcional): onde o botão de stream aparece na tela de cada jogo - predefinições rápidas ou ajuste fino por campo (top/bottom/left/right).
4. **Jogue**: o botão de satélite aparece na tela de cada jogo (ao lado do "Jogar" nativo) e no Quick Access. Ele detecta o contexto atual, aplica o perfil correspondente, configura o Apollo via API e lança o Moonlight através de um atalho Steam (necessário pro Gamescope focar a janela corretamente).
5. **Fechar conexão** (Quick Access): encerra a sessão no Apollo, restaura os displays do host à configuração original.
6. **Logs** (aba "Logs" nas configurações): mostra as últimas linhas do log da sessão atual do plugin, sem precisar de SSH.

## Desenvolvimento

```bash
pnpm i
pnpm run build       # build do frontend (dist/index.js)
./deploy.sh           # sincroniza com o Deck e reinicia o plugin_loader
./deploy.sh build     # builda e sincroniza numa tacada só
```

`deploy.sh` espera uma chave SSH sem senha pro Deck e algumas regras `sudoers` NOPASSWD - ver comentários no topo do script.

Documentação completa (motivação, arquitetura, fases de execução, decisões e limitações conhecidas) em [`docs/prd.md`](docs/prd.md).
