import { useState, useEffect } from "react";
import { PanelSection, PanelSectionRow, ButtonItem, Field, ProgressBarItem } from "@decky/ui";
import { toaster } from "@decky/api";
import { getProfiles, detectContext, stopStream } from "./api";
import { syncHostGames } from "./gameSync";
import { Profile } from "./types";

interface SyncProgress {
  current: number;
  total: number;
  gameName: string;
}

export function QuickAccessContent() {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [context, setContext] = useState<string>("...");
  const [closing, setClosing] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [syncProgress, setSyncProgress] = useState<SyncProgress | null>(null);

  useEffect(() => {
    getProfiles().then(setProfiles);
    detectContext().then(setContext);
  }, []);

  const onClose = async () => {
    setClosing(true);
    try {
      const result = await stopStream();
      if (result.ok) {
        toaster.toast({ title: "MoonProfile", body: "Conexao fechada" });
      } else {
        toaster.toast({ title: "MoonProfile - erro", body: result.error ?? "Falha desconhecida" });
      }
    } catch (e) {
      console.error("MoonProfile: erro inesperado ao fechar", e);
      toaster.toast({ title: "MoonProfile - erro inesperado", body: String(e) });
    } finally {
      setClosing(false);
    }
  };

  const onSyncGames = async () => {
    setSyncing(true);
    setSyncProgress(null);
    try {
      await syncHostGames((current, total, gameName) => setSyncProgress({ current, total, gameName }));
    } catch (e) {
      console.error("MoonProfile: erro inesperado sincronizando jogos", e);
      toaster.toast({ title: "MoonProfile - erro inesperado", body: String(e) });
    } finally {
      setSyncing(false);
      setSyncProgress(null);
    }
  };

  return (
    <>
      <PanelSection title="MoonProfile">
        <PanelSectionRow>
          <Field label="Contexto detectado">{context}</Field>
        </PanelSectionRow>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onClose} disabled={closing}>
            {closing ? "Fechando..." : "Fechar conexao"}
          </ButtonItem>
        </PanelSectionRow>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onSyncGames} disabled={syncing}>
            {syncing ? "Sincronizando..." : "Sincronizar jogos do host"}
          </ButtonItem>
        </PanelSectionRow>
        {syncProgress && (
          <>
            <PanelSectionRow>
              <Field label="Sincronizando">{`${syncProgress.gameName} (${syncProgress.current}/${syncProgress.total})`}</Field>
            </PanelSectionRow>
            <PanelSectionRow>
              {/* So' a barra, sem sOperationText/sTimeRemaining -
                  ProgressBarWithInfo estourava esse texto pra fora do
                  painel estreito do Quick Access (confirmado por
                  screenshot: cortado no canto direito). O texto acima
                  (Field) ja cobre a mesma informacao, sem esse problema. */}
              <ProgressBarItem nProgress={(syncProgress.current / syncProgress.total) * 100} />
            </PanelSectionRow>
          </>
        )}
      </PanelSection>

      <PanelSection title="Perfis">
        {profiles.length === 0 && <PanelSectionRow>Nenhum perfil configurado</PanelSectionRow>}
        {profiles.map((p) => (
          <PanelSectionRow key={p.id}>
            <Field label={p.name}>{p.trigger}</Field>
          </PanelSectionRow>
        ))}
      </PanelSection>
    </>
  );
}
