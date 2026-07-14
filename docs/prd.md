# MoonProfile

Plugin Decky Loader para Steam Deck que gerencia perfis de streaming Moonlight com detecção automática de contexto (docked vs handheld) e configuração dinâmica do host Apollo via API REST.

## Motivação

Fluxo atual de streaming via Moonlight sofre de:

- Moonlight não conhece contexto de uso (docked/handheld), dispara resoluções erradas (ex: 800p ao invés de 4K quando dockado)
- Apollo prep-cmd fixo não se adapta a diferentes cenários (HDR TV vs SDR handheld)
- MoonDeck resolve parte do problema, mas exige daemon extra no host (Buddy) e não tem perfis contextuais
- Configurar manualmente a cada sessão (bitrate, codec, HDR, output alvo) é insustentável

O plugin centraliza as decisões que hoje estão espalhadas entre Moonlight, Apollo, KDE, Steam e o usuário.

## Diferencial em relação ao MoonDeck

- Zero componente adicional no host (usa API REST nativa do Apollo)
- Perfis de streaming editáveis in-place no Deck
- Detecção automática de contexto (docked/handheld)
- Cada perfil controla simultaneamente configuração de cliente Moonlight e configuração de displays no host

## Stack

- **Frontend**: TypeScript, React, `@decky/ui`, `@decky/api`
- **Backend**: Python 3.11+ (embutido no Decky Loader)
- **Bundler**: Rollup
- **Host requirements**: Apollo 0.4.8+, KDE Plasma 6 Wayland, GPU AMD RDNA 4 ou compatível (via VAAPI)
- **Cliente**: Moonlight Flatpak (`com.moonlight_stream.Moonlight`)

## Arquitetura

```
[Deck: biblioteca do Steam]
    ↓
[Quick Access ou botão na tela do jogo]
    ↓
[Backend Python do plugin]
    ├─→ Detecta contexto (docked/handheld) via /sys/class/drm
    ├─→ Seleciona perfil correspondente
    ├─→ POST na API do Apollo: atualiza app "SteamGame" com prep-cmd + cmd
    └─→ subprocess: Moonlight CLI com args do perfil
         ↓
[Apollo executa prep-cmd DO com args do perfil]
    ├─→ Ativa output alvo (HDMI-A-1, DP-3, etc)
    ├─→ Configura resolução e refresh rate
    ├─→ Ativa HDR e WCG se aplicável
    ├─→ Desabilita outros outputs
    └─→ Executa steam://rungameid/APPID
         ↓
    [Stream rodando]
         ↓
[Ao fechar Moonlight ou perder conexão]
    ↓
[Apollo executa prep-cmd UNDO]
    ├─→ pkill do processo do jogo pelo AppID
    ├─→ Restaura outputs originais
    └─→ Desativa output virtual
```

## Modelo de dados

### Perfil

```typescript
interface Profile {
    id: string;                    // ex: "docked-tv-4k-hdr"
    name: string;                  // ex: "Docked TV 4K HDR"
    trigger: "docked" | "handheld" | "manual";
    moonlight: MoonlightConfig;
    host: HostConfig;
}

interface MoonlightConfig {
    resolution: string;            // ex: "3840x2160"
    fps: number;                   // ex: 60
    bitrate: number;               // em kbps, ex: 150000
    codec: "HEVC" | "AV1" | "H264";
    hdr: boolean;
}

interface HostConfig {
    target_output: string;         // ex: "HDMI-A-1"
    resolution: string;            // ex: "3840x2160"
    fps: number;                   // ex: 60
    hdr: boolean;
    wcg: boolean;                  // Wide Color Gamut
    disable_outputs: string[];     // ex: ["DP-3"]
}
```

### Config global

```typescript
interface Config {
    host: string;                  // ex: "192.168.1.6"
    username: string;              // credencial admin do Apollo
    password: string;              // credencial admin do Apollo
}
```

Persistência:
- `$DECKY_PLUGIN_SETTINGS_DIR/profiles.json`
- `$DECKY_PLUGIN_SETTINGS_DIR/config.json` (permissões 0600)

## Estrutura do repositório

```
moonprofile/
├── plugin.json                   # metadata Decky
├── package.json                  # deps frontend
├── rollup.config.js              # bundler
├── tsconfig.json
├── main.py                       # backend Python
├── src/
│   ├── index.tsx                 # entry point + registro de patches
│   ├── types.ts                  # interfaces compartilhadas
│   ├── api.ts                    # bindings callable() do backend
│   ├── QuickAccessContent.tsx    # UI principal
│   ├── ProfileEditor.tsx         # editor CRUD de perfis
│   ├── ConfigEditor.tsx          # config global (host, credenciais)
│   ├── GameActionButton.tsx      # botão custom pra tela do jogo (Fase 3)
│   └── patches/
│       └── LibraryAppPatch.tsx   # patch React pra injetar botão (Fase 3)
├── defaults/                     # arquivos default do primeiro run
│   └── profiles.json             # perfis de exemplo
└── PROJECT.md                    # este arquivo
```

## Fases de execução

### Fase 0: Prova de conceito CLI (target: 1h)

Valida a arquitetura sem escrever plugin.

Objetivos:
- Via curl, atualizar app "SteamGame" no Apollo com prep-cmd customizado
- Via Moonlight CLI, conectar na app atualizada
- Confirmar que HDR, resolução e AppID dinâmico funcionam ponta a ponta

Entregável: script bash de referência que reproduz o fluxo completo.

Critério de sucesso: consegue lançar RE4 com HDR ativo via linha de comando, atualizar pra outro AppID sem reiniciar Apollo.

### Fase 1: Backend Python + Quick Access mínimo (target: 3h)

Plugin funcional com config e um perfil hardcoded.

Objetivos:
- Clone do template Decky
- `main.py` completo com métodos: `get_config`, `save_config`, `get_profiles`, `save_profiles`, `detect_context`, `stream_game`
- UI Quick Access com: config global editável + lista de perfis + botão "Stream currently selected game"
- Pega AppID do jogo em foco via `SteamClient.Router.MainRunningApp` ou similar
- Perfis hardcoded no `defaults/profiles.json`

Entregável: plugin instalável no Deck que substitui MoonDeck no fluxo docked/handheld.

Critério de sucesso: seleciona jogo na biblioteca, abre Quick Access, clica "Stream", contexto detectado corretamente, jogo lança no host com perfil aplicado.

### Fase 2: UI de perfis (target: 3h)

Editor CRUD de perfis dentro do Quick Access.

Objetivos:
- Criar, editar, duplicar, deletar perfis
- Todos os campos editáveis via TextField, DropdownItem, SliderField, ToggleField
- Validação básica (nome único, resolução no formato correto)
- Feedback visual (toaster.toast) em cada operação

Entregável: gerenciamento completo de perfis sem editar JSON manualmente.

Critério de sucesso: cria um perfil novo do zero, salva, aplica em um stream, sem tocar em arquivo.

### Fase 3: Botão na tela do jogo (target: 2-6h, imprevisível)

Injeção via patch React na página de detalhes do jogo.

Objetivos:
- `routerHook.addPatch("/library/app/:appid", ...)`
- `afterPatch` e `findInReactTree` pra localizar o container de ações
- Injeta `StreamButton` que chama `streamGame(appId, gameName)`
- Dropdown pra escolher perfil manualmente (opcional)

Entregável: botão "Stream via Moonlight" aparece na tela de cada jogo, ao lado dos botões padrão.

Critério de sucesso: clica direto no botão sem passar por Quick Access, stream inicia.

Risco: parte mais frágil, quebra entre versões do Steam client. Estudar código atual do MoonDeck é obrigatório.

### Fase 4: Polish

Objetivos (sem ordem específica, escolher conforme uso real):
- ~~Notificações persistentes durante stream ativo / detecção de fim de sessão~~ - movido pra Fase 5 (precisa do daemon no host, ver abaixo). A ideia original de pollar `current_app` do Apollo **não funciona** - motivo documentado na Fase 5.
- ✅ Tratamento de erro (host offline, credenciais erradas, Apollo não respondendo) - `main.py:_apollo_error_response`, diferencia os 3 casos (confirmado 401 = credencial errada lendo `confighttp.cpp` do Apollo).
- ✅ Ícone customizado no menu do Decky (`FaSatelliteDish`, já feito)
- ✅ Logs internos acessíveis pela UI - aba "Logs" na sidenav de Configurações, lê `decky.DECKY_PLUGIN_LOG` sob demanda.
- ❌ Descartado: detecção de OLED vs LCD do Deck - sem caso de uso concreto que justifique (só mudaria defaults de FPS/HDR no perfil handheld; usuário já configura isso manualmente sem problema).
- ❌ Descartado por agora: suporte a múltiplos hosts - usuário só usa um host Apollo hoje, sem necessidade real. Reconsiderar se isso mudar.

Fase 4 encerrada com o que fazia sentido implementar agora.

### Fase 4.5: Suporte a jogos non-Steam e atalhos por jogo

Duas features levantadas em uso real, adiadas de propósito (aumentam bastante o escopo pra valer a pena antes de validar o resto no dia a dia). Ambas estudadas contra o código do MoonDeck/MoonDeck Buddy, sem adotar a arquitetura deles (ver decisão abaixo).

Objetivos:
- **Suporte a jogos non-Steam.** Hoje `stream_game(app_id)` usa o MESMO id pra achar a página no Deck (patch da Fase 3) e pra montar o `cmd`/`pkill` no host (`main.py:216,219`). Isso só funciona porque appids de jogos de catálogo Steam são globais (mesmo número em qualquer instalação) - pra um atalho non-Steam (Epic, GOG, emulador) o id é um hash local, diferente em cada máquina. Fix: separar `deck_app_id` (só serve pra achar o perfil/página no Deck) de `host_app_id` (usado no `cmd`/`pkill` do Apollo) no schema de perfil, com o segundo como campo opcional preenchido manualmente quando o jogo não é Steam "de verdade". Convenção pro usuário: criar o atalho non-Steam no client Steam do HOST primeiro (igual precisaria fazer pra usar Sunshine/Apollo mesmo sem o MoonProfile).
- **Atalhos por jogo + "Jogado recentemente".** Trocar o atalho Steam compartilhado (`steamShortcut.ts`) por um atalho por jogo faria o Deck mostrar cada jogo streamado separadamente em "Jogado recentemente", em vez de um único "MoonProfile Launcher" genérico. Estudado o mecanismo real do MoonDeck (`AppOverviewPatcher`, `MoonDeckAppShortcuts`): não é só criar N atalhos, eles mantêm um `BiMap` atalho↔jogo e fazem monkey-patch ao vivo do campo `rt_last_time_locally_played` no app store da Steam via `appStoreEx.observe()`/`intercept()`, com detecção de corrupção de cache e purga+reinício do client Steam como fallback. Genuinamente um dos subsistemas mais complexos do MoonDeck inteiro - vale a pena, mas não é trivial.

Decisão explícita (registrada pra não repetir a discussão depois): **não forkar o MoonDeck nem o Buddy.** A arquitetura deles pressupõe exatamente os dois itens que este projeto existe pra evitar (daemon extra no host via Buddy, ausência de perfis contextuais - ver Motivação). Forkar herdaria a dependência do Buddy e ainda exigiria enxertar perfis automáticos num código C++/TS desconhecido - mais trabalho, não menos. A estratégia continua sendo: ler o código deles como referência pontual (como já feito pro botão da tela do jogo e pro fix do `gameid`), implementar direto no stack Apollo + Python + React já validado.

### Fase 5: MoonProfile Buddy (daemon no host)

Mudança de arquitetura deliberada, adiada pra ser projetada com calma numa fase própria - **não é só mais um item da Fase 4**, é abrir mão do diferencial "zero componente adicional" (Motivação/Diferencial, no topo deste documento) em troca de robustez real.

**Por que isso passou a ser necessário** (achado técnico, não repetir a investigação): tentamos resolver detecção de fim de sessão via *polling* de `GET /api/apps` (campo `current_app`), a solução que a Fase 4 original previa. Não funciona. Lendo o código do Apollo (`ClassicOldSong/Apollo`, `src/process.cpp`, função `proc_t::running()`):

```cpp
} else if (_app.auto_detach && std::chrono::steady_clock::now() - _app_launch_time < 5s) {
  // "App exited within 5 seconds of launch. Treating the app as a detached command."
  placebo = true;
  return _app_id;  // dai em diante, "rodando" pra sempre
}
```

Nosso `stream_game()` usa `"auto-detach": true` justamente porque `cmd: "steam steam://rungameid/{app_id}"` retorna quase na hora (é só um relay pro client Steam - o jogo real roda solto, desprendido). Isso é exatamente o gatilho do `placebo = true`: uma vez nesse modo, `running()` **nunca mais volta a zero sozinho**, então `current_app` fica preso "rodando" até alguém chamar `close_app` manualmente (nosso "Fechar conexão"). Não tem workaround de polling que resolva isso - o dado que estaríamos lendo simplesmente não reflete a realidade.

**O que um daemon no host resolveria de verdade:**
- Detecção de fim de sessão checando o processo diretamente (`pgrep`/API nativa do SO), independente do `placebo`/`auto-detach` do Apollo.
- Enumeração de jogos non-Steam do host (sem depender do usuário criar atalhos manualmente - resolve boa parte da Fase 4.5 de outra forma).
- Checagem de prontidão do host antes de iniciar o stream (GPU/encoder disponível, sessão Plasma ativa, etc).

**Alternativa mais barata considerada e descartada por ora:** trocar `cmd` por um script estático (não um serviço) que bloqueia até o jogo fechar de verdade, tirando o app do modo `placebo` sem precisar de um daemon persistente. Resolveria só a detecção de fim de sessão, não as outras duas features. Decisão do usuário: vale mais a pena projetar o daemon completo numa fase própria do que resolver só uma parte agora.

**Ainda em aberto (a decidir quando essa fase começar):** linguagem/stack do daemon (Python stdlib, consistente com o resto do projeto, é o candidato natural), protocolo de comunicação com o plugin (REST próprio? reusar alguma coisa do Apollo?), autenticação/pareamento, mecanismo de instalação (serviço systemd do usuário, não root), e o que exatamente ele expõe primeiro.

## Referências técnicas

### API do Apollo (herdada do Sunshine)

Endpoint: `https://HOST:47990/api/apps`

Autenticação: Basic auth (admin/senha configurados no Apollo).

Certificado auto-assinado, cliente precisa desabilitar verificação SSL.

Non-browser clients são isentos de CSRF (confirmado na doc oficial).

Métodos usados:
- `GET /api/apps` → lista apps atuais
- `POST /api/apps` → cria ou atualiza (usar `index: -1` pra criar, ou índice existente pra atualizar)

Corpo do POST:

```json
{
  "name": "SteamGame",
  "cmd": "steam steam://rungameid/2050650",
  "index": -1,
  "auto-detach": true,
  "wait-all": false,
  "exit-timeout": 5,
  "exclude-global-prep-cmd": false,
  "elevated": false,
  "prep-cmd": [{
    "do": "bash -c '...comando inline...'",
    "undo": "bash -c '...comando inline...'"
  }],
  "output": "/tmp/apollo-steamgame-2050650.log"
}
```

Limitação conhecida: campo `env` só é editável via arquivo `apps.json` direto, não via API. Por isso passamos tudo via `prep-cmd` inline.

### Comando de undo com kill limpo do jogo

```bash
# gerado dinamicamente pelo plugin, embarcando o AppID conhecido
pkill -TERM -f "AppId=2050650" ; sleep 5 ; pkill -KILL -f "AppId=2050650" 2>/dev/null ; setsid steam steam://close/bigpicture ; sleep 2 ; kscreen-doctor output.DP-3.enable ; sleep 1 ; kscreen-doctor output.HDMI-A-1.disable
```

Uso de `;` em vez de `&&` é intencional: se pkill retorna erro (jogo já fechou), a cadeia continua e restaura os displays.

### Detecção de contexto

```python
def detect_context() -> str:
    """Retorna 'docked' se algum display externo tá conectado, senão 'handheld'."""
    drm_path = "/sys/class/drm"
    for entry in os.listdir(drm_path):
        if not entry.startswith("card"):
            continue
        if not ("HDMI" in entry or "DP" in entry):
            continue
        status_file = os.path.join(drm_path, entry, "status")
        if os.path.exists(status_file):
            with open(status_file) as f:
                if f.read().strip() == "connected":
                    return "docked"
    return "handheld"
```

### Steam Browser Protocol

Existentes e usados:
- `steam://rungameid/<appid>` → lança jogo
- `steam://open/bigpicture` → abre Big Picture
- `steam://close/bigpicture` → fecha Big Picture

NÃO existe:
- `steam://exit/<appid>` → não é URL scheme válido, motivo pelo qual usamos `pkill`

## Fluxo de desenvolvimento

### Setup inicial

```bash
git clone https://github.com/SteamDeckHomebrew/decky-plugin-template moonprofile
cd moonprofile
rm -rf .git && git init
pnpm install
```

Edita `plugin.json` com nome, autor, descrição.

### Build

```bash
pnpm build
```

Gera `dist/index.js` que o Decky Loader carrega.

### Deploy no Deck

Método rsync:
```bash
rsync -avz --delete \
    ./ deck@STEAMDECK_IP:/home/deck/homebrew/plugins/moonprofile/ \
    --exclude node_modules --exclude .git

ssh deck@STEAMDECK_IP "systemctl --user restart plugin_loader"
```

Método VS Code: Remote-SSH direto no Deck, edita in-place, reload pela UI do Decky.

### Logs

No Deck:
```bash
journalctl --user -f | grep -i decky
```

Logs do plugin especificamente:
```bash
tail -f /home/deck/homebrew/logs/moonprofile/plugin.log
```

Frontend logs vão pro Steam WebHelper devtools (habilitar via Decky Settings → Developer Options).

## Riscos e limitações conhecidas

1. **Patch da biblioteca é frágil**: nomes de classe React do Gaming Mode mudam entre versões do Steam client. Manutenção obrigatória. Mitigação: começar sem patch (só Quick Access), adicionar depois se realmente necessário.

2. **Escape de strings no prep-cmd**: se caminho ou nome de perfil tiver aspas simples, quebra. Mitigação: sanitizar inputs no editor.

3. **Sem sincronização de saves além do Steam Cloud**: aceitável pro fluxo pessoal.

4. **Sem retomada automática de sessão**: se cair a conexão, reabre manualmente.

5. **`sleep 5` no undo pode não ser suficiente pra jogos com autosave raro**: aceitar perda ou aumentar. Configurável por perfil na Fase 4.

6. **Trigger `docked` sozinho não distingue rede boa vs ruim**: se você joga docked em casa e docked na casa de amigo, precisa selecionar perfil manualmente. Ampliar pra trigger composto (docked + SSID) é possível na Fase 4.

7. **Match por `AppId=` no pkill é frágil se dois jogos rodam simultaneamente**: cenário raro.

## Recursos externos

- Sunshine/Apollo API: https://docs.lizardbyte.dev/projects/sunshine/latest/md_docs_2api.html
- MoonDeck (case study): https://github.com/FrogTheFrog/moondeck
- Decky Loader wiki: https://wiki.deckbrew.xyz/en/plugin-dev/getting-started
- Decky plugin template: https://github.com/SteamDeckHomebrew/decky-plugin-template
- HLTB plugin (referência de patch simples): https://github.com/OMGDuke/HLTB-For-Deck

## Restrições de escopo (importante)

**Hard stop na Fase 1.** Uso real por 2 semanas antes de decidir Fase 2 ou 3.

Motivos:
- Padrão histórico de acumular projetos parciais
- Prazo do Ares em agosto tem prioridade sobre este projeto
- Rewrite do Oráculo em andamento não pode desacelerar
- Fase 1 já resolve o problema pessoal (docked/handheld com perfis)
- Fase 2 e 3 são polish, não features essenciais

Se após 2 semanas de uso real houver dor genuína (não vontade abstrata) por CRUD de perfis ou botão na tela do jogo, aí sim investir mais tempo. Antes disso, é sinal de over-engineering ou procrastinação disfarçada.