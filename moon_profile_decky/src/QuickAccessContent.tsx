import { useState, useEffect } from "react";
import { PanelSection, PanelSectionRow, ButtonItem, Field } from "@decky/ui";
import { toaster } from "@decky/api";
import { getProfiles, detectContext, stopStream } from "./api";
import { syncHostGames } from "./gameSync";
import { Profile } from "./types";

interface SyncProgress {
  current: number;
  total: number;
  gameName: string;
}

// Custom progress bar, pure CSS: the ready-made @decky/ui components
// (ProgressBarWithInfo, ProgressBarItem) overflowed the Quick Access
// panel's narrow width in two different ways (confirmed by screenshot:
// first the text, then the bar box itself), even with layout="below". A
// <div> with a percentage width doesn't have that problem, it's guaranteed
// by CSS and doesn't depend on the internal behavior (possibly buggy in
// this context) of Steam's component.
function ProgressBar({ percent }: { percent: number }) {
  return (
    <div style={{ width: "100%", height: "4px", background: "rgba(255, 255, 255, 0.2)", borderRadius: "2px" }}>
      <div
        style={{
          width: `${percent}%`,
          height: "100%",
          background: "#67c1f5",
          borderRadius: "2px",
          transition: "width 0.2s ease-out",
        }}
      />
    </div>
  );
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
        toaster.toast({ title: "MoonProfile", body: "Connection closed" });
      } else {
        toaster.toast({ title: "MoonProfile - error", body: result.error ?? "Unknown failure" });
      }
    } catch (e) {
      console.error("MoonProfile: unexpected error while closing", e);
      toaster.toast({ title: "MoonProfile - unexpected error", body: String(e) });
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
      console.error("MoonProfile: unexpected error syncing games", e);
      toaster.toast({ title: "MoonProfile - unexpected error", body: String(e) });
    } finally {
      setSyncing(false);
      setSyncProgress(null);
    }
  };

  return (
    <>
      <PanelSection title="MoonProfile">
        <PanelSectionRow>
          <Field label="Detected context">{context}</Field>
        </PanelSectionRow>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onClose} disabled={closing}>
            {closing ? "Closing..." : "Close connection"}
          </ButtonItem>
        </PanelSectionRow>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onSyncGames} disabled={syncing}>
            {syncing ? "Syncing..." : "Sync games from host"}
          </ButtonItem>
        </PanelSectionRow>
        {syncProgress && (
          <>
            <PanelSectionRow>
              <Field label="Syncing">{`${syncProgress.gameName} (${syncProgress.current}/${syncProgress.total})`}</Field>
            </PanelSectionRow>
            <PanelSectionRow>
              <ProgressBar percent={(syncProgress.current / syncProgress.total) * 100} />
            </PanelSectionRow>
          </>
        )}
      </PanelSection>

      <PanelSection title="Profiles">
        {profiles.length === 0 && <PanelSectionRow>No profile configured</PanelSectionRow>}
        {profiles.map((p) => (
          <PanelSectionRow key={p.id}>
            <Field label={p.name}>{p.trigger}</Field>
          </PanelSectionRow>
        ))}
      </PanelSection>
    </>
  );
}
