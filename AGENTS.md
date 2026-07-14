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

Ainda não tem harness de teste automatizado configurado. Verificação hoje é
manual: `npm run build`, `python3 -c "import ast; ast.parse(...)"` pro
backend, e teste no device via `./deploy.sh`. Se/quando um harness for
adicionado (ex: vitest pro frontend, pytest pro `main.py`), a mesma regra
deste arquivo passa a valer aqui também - features novas vêm com teste.
