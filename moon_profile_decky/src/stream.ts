import { toaster } from "@decky/api";
import { streamGame, stopStream, checkSessionStatus } from "./api";
import { ensureLauncherShortcut, launchViaSteam } from "./steamShortcut";

const SESSION_POLL_MS = 5000;
let pollIntervalId: ReturnType<typeof setInterval> | null = null;

// Chamado tanto quando o proprio poll detecta que a sessao acabou quanto
// quando o usuario clica "Fechar conexao" na mao (QuickAccessContent.tsx) -
// evita continuar pollando (e potencialmente disparando um stopStream()
// redundante) depois que a sessao ja foi encerrada de um jeito ou de outro.
export function stopSessionWatch(): void {
  if (pollIntervalId !== null) {
    clearInterval(pollIntervalId);
    pollIntervalId = null;
  }
}

// Fase 5: o Apollo nao detecta sozinho quando o jogo fecha por dentro (sem
// desconectar o Moonlight) - auto-detach do stream_game entra em modo
// "placebo" no Apollo (ver main.py/docs/prd.md), current_app nunca mais
// reflete a realidade. O Deck pergunta pro MoonProfile Runner (daemon no
// host) se o processo ainda esta vivo. Se o runner nao estiver configurado,
// o backend sempre responde running=true (fallback seguro) - o polling
// roda mas nunca dispara nada, custo desprezivel.
function watchSession(appId: number): void {
  stopSessionWatch();
  pollIntervalId = setInterval(async () => {
    const status = await checkSessionStatus(appId);
    if (!status.running) {
      stopSessionWatch();
      await stopStream();
      toaster.toast({ title: "MoonProfile", body: "Sessao encerrada (jogo fechado)" });
    }
  }, SESSION_POLL_MS);
}

// Fluxo compartilhado entre o Quick Access e o botao na tela do jogo (Fase
// 3): resolve o perfil no backend, garante o atalho Steam "MoonProfile
// Launcher", dispara o lancamento por ele. Ver stream.py:stream_game() e
// steamShortcut.ts pro resto do mecanismo.
export async function runStream(appId: number): Promise<void> {
  const result = await streamGame(appId);
  if (!result.ok || !result.runner_path || !result.launch_env) {
    toaster.toast({ title: "MoonProfile - erro", body: result.error ?? "Falha desconhecida" });
    return;
  }

  const shortcutAppId = await ensureLauncherShortcut(result.runner_path);
  if (shortcutAppId === null) {
    toaster.toast({
      title: "MoonProfile - erro",
      body: "Nao consegui criar/validar o atalho do Moonlight na Steam",
    });
    return;
  }

  await launchViaSteam(shortcutAppId, result.launch_env);
  watchSession(appId);
  toaster.toast({
    title: "MoonProfile",
    body: `Streaming com perfil "${result.profile}" (${result.context})`,
  });
}
