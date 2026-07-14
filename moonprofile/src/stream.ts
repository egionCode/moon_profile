import { toaster } from "@decky/api";
import { streamGame } from "./api";
import { ensureLauncherShortcut, launchViaSteam } from "./steamShortcut";

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
  toaster.toast({
    title: "MoonProfile",
    body: `Streaming com perfil "${result.profile}" (${result.context})`,
  });
}
