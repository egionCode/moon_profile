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
- Notificações persistentes durante stream ativo
- Tratamento de erro (host offline, credenciais erradas, Apollo não respondendo)
- Ícone customizado no menu do Decky
- Detecção de OLED vs LCD do Deck (se relevante pros perfis)
- Suporte a múltiplos hosts (não apenas um)
- Logs internos acessíveis pela UI

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