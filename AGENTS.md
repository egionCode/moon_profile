# AGENTS.md

Regra pra qualquer agente (Claude Code ou outro) trabalhando neste monorepo.

## Toda feature nova precisa vir com teste automatizado

Ao criar ou alterar uma feature, adicione (ou atualize) os testes que cobrem
o comportamento novo/mudado **na mesma sessão de trabalho**, não depois.

Preferir testar comportamento real em vez de mockar: já encontramos bugs
reais dessa forma (ver `moon_profile_runner/src-tauri/src/server.rs`, testes
que spawnam um processo de verdade em vez de simular `sysinfo` - pegaram um
bug de refresh de processo E um de match por prefixo compartilhado, ambos
que um mock não pegaria).

## moon_profile_runner/ (Tauri/Rust)

- Testes vivem em `#[cfg(test)] mod tests` dentro do próprio arquivo do
  código testado (ex: `src-tauri/src/server.rs`).
- Rodar com `cargo test` (dentro de `moon_profile_runner/src-tauri/`).
- Para endpoints HTTP: usar `tower::ServiceExt::oneshot` direto no `Router`
  (sem precisar abrir uma porta TCP de verdade - rápido e sem conflito de
  porta entre execuções).
- Para lógica que depende do SO (detecção de processo, etc.): preferir
  spawnar um processo/recurso real de teste em vez de mockar a API do SO -
  é assim que os bugs reais foram encontrados aqui.
- Toda função helper "pura" (sem I/O) ganha também um teste unitário rápido
  separado do teste de integração (ex: `cmd_arg_matches_app_id_cases`).

## moon_profile_decky/ (plugin Decky - TypeScript + Python)

### Frontend (TypeScript)

- Harness: `vitest`. Rodar com `npm run test` (ou `npx vitest run`) dentro
  de `moon_profile_decky/`.
- Testes vivem em `tests/*.test.ts`, ambiente Node puro (não jsdom - o
  código sob teste só toca a superfície de globals que a Steam injeta em
  `window`, ex: `SteamClient`, `appStore`, `collectionStore`; nunca
  renderiza DOM de verdade). `tests/setup.ts` garante que `window` existe
  em Node antes de qualquer teste rodar.
- `SteamClient`/`appStore`/`collectionStore` não são tipados por
  `@decky/ui` (API não documentada) - mocka-los direto em `window` com
  `vi.fn()` por teste, sem depender de um cliente Steam de verdade.
- Módulos que chamam `callable(...)` do `@decky/api` (ver `src/api.ts`) -
  mockar o módulo inteiro via `vi.mock("../src/api", ...)` no teste (ver
  `tests/gameCollection.test.ts` pro padrão).
- `npm run build` (`tsc` via rollup) continua sendo a verificação de tipo
  real - `tsc --noEmit` solto na raiz falha por causa de configs
  pré-existentes do projeto (react-router/JSX namespace), não é
  informativo sozinho.

### Backend (Python)

- Harness: `pytest` (+ `pytest-asyncio`, `asyncio_mode = auto` via
  `pytest.ini`). Ambiente isolado em `.venv/` (não versionado) - criar
  com `python3 -m venv .venv && .venv/bin/pip install -r
  requirements-dev.txt`, rodar com `.venv/bin/python -m pytest tests/`.
- `main.py` só existe dentro do runtime do Decky Loader de verdade (que
  injeta `py_modules/` no `sys.path` e um módulo global `decky` com
  diretórios/logger) - `tests/conftest.py` recria isso artificialmente
  (fixture `plugin_module`) importando `main.py` com um módulo `decky`
  falso apontando pra uma pasta temporária isolada por teste, nunca a
  config de verdade do usuário.
- `moonprofile_core.py` (lógica compartilhada com `runner.py`) é testado
  direto, sem precisar do fake `decky` - é código puro/stdlib.
- Funções que dependem do SO (ex: `detect_context`, que lê
  `/sys/class/drm`) recebem o caminho como parâmetro (default pro caminho
  real) especificamente pra permitir testar contra uma fixture em vez do
  hardware de verdade da máquina rodando o teste.
