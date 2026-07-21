import { useState, useEffect } from "react";
import { PanelSection, PanelSectionRow, ButtonItem, Field, showModal, ConfirmModal } from "@decky/ui";
import { toaster } from "@decky/api";
import { getProfiles, detectContext, stopStream, getHostStatus, shutdownHost, wakeHost } from "./api";
import { syncHostGames } from "./gameSync";
import { HostStatus, Profile } from "./types";

const HOST_STATUS_LABELS: Record<HostStatus, string> = {
  unconfigured: "Not configured",
  online: "Online",
  offline: "Offline",
};

// Polling interval for GET /health via get_host_status - frequent enough
// for the indicator to feel live, without hammering the Runner.
const HOST_STATUS_POLL_INTERVAL_MS = 5000;

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
  const [hostStatus, setHostStatus] = useState<HostStatus>("unconfigured");
  const [powerBusy, setPowerBusy] = useState(false);

  useEffect(() => {
    getProfiles().then(setProfiles);
    detectContext().then(setContext);
  }, []);

  useEffect(() => {
    const poll = () => getHostStatus().then(setHostStatus);
    poll();
    const interval = setInterval(poll, HOST_STATUS_POLL_INTERVAL_MS);
    return () => clearInterval(interval);
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

  const onWake = async () => {
    setPowerBusy(true);
    try {
      const result = await wakeHost();
      if (result.ok) {
        toaster.toast({ title: "MoonProfile", body: "Wake-on-LAN packet sent" });
      } else {
        toaster.toast({ title: "MoonProfile - error", body: result.error ?? "Unknown failure" });
      }
    } catch (e) {
      console.error("MoonProfile: unexpected error waking the host", e);
      toaster.toast({ title: "MoonProfile - unexpected error", body: String(e) });
    } finally {
      setPowerBusy(false);
    }
  };

  // Gated behind ConfirmModal: destructive and hard to reverse without
  // Wake-on-LAN already configured and working on the host's BIOS/NIC
  // (outside this code's control, see docs/prd.md Phase 6).
  const onShutdown = () => {
    showModal(
      <ConfirmModal
        strTitle="Turn off host"
        strDescription="This will shut down the host PC. Make sure Wake-on-LAN is set up if you want to turn it back on remotely."
        onOK={async () => {
          setPowerBusy(true);
          try {
            const result = await shutdownHost();
            if (result.ok) {
              toaster.toast({ title: "MoonProfile", body: "Shutdown requested" });
            } else {
              toaster.toast({ title: "MoonProfile - error", body: result.error ?? "Unknown failure" });
            }
          } catch (e) {
            console.error("MoonProfile: unexpected error shutting down the host", e);
            toaster.toast({ title: "MoonProfile - unexpected error", body: String(e) });
          } finally {
            setPowerBusy(false);
          }
        }}
      />,
    );
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
          <Field label="Host status">{HOST_STATUS_LABELS[hostStatus]}</Field>
        </PanelSectionRow>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onClose} disabled={closing}>
            {closing ? "Closing..." : "Close connection"}
          </ButtonItem>
        </PanelSectionRow>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onShutdown} disabled={powerBusy || hostStatus !== "online"}>
            Turn off host
          </ButtonItem>
        </PanelSectionRow>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onWake} disabled={powerBusy || hostStatus !== "offline"}>
            Wake host
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
