// "Games" tab of the Settings sidenav: a grid showing the per-game
// shortcuts already synced (see gameSync.ts), with cover art when
// available (only for real Steam games for now, Stage A). Creating a
// shortcut is still only done via the "Sync games from host" button in
// Quick Access; the "Clear" button here removes everything (from Steam
// and from the persisted file).
import { CSSProperties, useEffect, useState } from "react";
import { ButtonItem, Focusable, PanelSection, PanelSectionRow } from "@decky/ui";
import { toaster } from "@decky/api";
import { getGameShortcuts, saveGameShortcuts } from "./api";
import { getImageAsB64, getSteamCapsuleUrl } from "./gameArtwork";
import { removeAllGameShortcuts } from "./gameShortcuts";
import { GameShortcuts } from "./types";

const gridStyle: CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fill, minmax(120px, 1fr))",
  gap: "12px",
  width: "100%",
};

const cardStyle: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "4px",
};

// Steam's vertical capsule is 2:3 (ex: 600x900), keep that aspect ratio
// even before the image loads, so the layout doesn't jump once it arrives.
const imageWrapperStyle: CSSProperties = {
  aspectRatio: "2 / 3",
  borderRadius: "6px",
  overflow: "hidden",
  backgroundColor: "rgba(255, 255, 255, 0.08)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  textAlign: "center",
  padding: "4px",
  fontSize: "0.75em",
  opacity: 0.8,
};

const imageStyle: CSSProperties = {
  width: "100%",
  height: "100%",
  objectFit: "cover",
};

const labelStyle: CSSProperties = {
  fontSize: "0.8em",
  textAlign: "center",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

interface GameCardProps {
  hostAppId: string;
  name: string;
  isSteam: boolean;
}

function GameCard({ hostAppId, name, isSteam }: GameCardProps) {
  const [imageSrc, setImageSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!isSteam) {
      return; // non-Steam doesn't have a cover art source yet (Stage B, SteamGridDB)
    }
    let cancelled = false;
    getImageAsB64(getSteamCapsuleUrl(hostAppId)).then((data) => {
      if (!cancelled && data) {
        setImageSrc(`data:image/jpeg;base64,${data}`);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [hostAppId, isSteam]);

  return (
    <div style={cardStyle}>
      <div style={imageWrapperStyle}>
        {imageSrc ? <img src={imageSrc} alt={name} style={imageStyle} /> : name}
      </div>
      <div style={labelStyle}>{name}</div>
    </div>
  );
}

export function GamesGridSection() {
  const [shortcuts, setShortcuts] = useState<GameShortcuts>({});
  const [loaded, setLoaded] = useState(false);
  const [clearing, setClearing] = useState(false);

  useEffect(() => {
    getGameShortcuts().then((s) => {
      setShortcuts(s);
      setLoaded(true);
    });
  }, []);

  const onClear = async () => {
    setClearing(true);
    try {
      removeAllGameShortcuts(shortcuts);
      await saveGameShortcuts({});
      setShortcuts({});
      toaster.toast({ title: "MoonProfile", body: "Synced games removed" });
    } catch (e) {
      console.error("MoonProfile: unexpected error clearing synced games", e);
      toaster.toast({ title: "MoonProfile - unexpected error", body: String(e) });
    } finally {
      setClearing(false);
    }
  };

  const entries = Object.entries(shortcuts);

  return (
    <PanelSection>
      {!loaded && <PanelSectionRow>Loading...</PanelSectionRow>}
      {loaded && entries.length === 0 && (
        <PanelSectionRow>
          No games synced yet, use &quot;Sync games from host&quot; in Quick Access.
        </PanelSectionRow>
      )}
      {entries.length > 0 && (
        <>
          <PanelSectionRow>
            <Focusable style={gridStyle}>
              {entries.map(([hostAppId, entry]) => (
                <GameCard key={hostAppId} hostAppId={hostAppId} name={entry.name} isSteam={entry.is_steam} />
              ))}
            </Focusable>
          </PanelSectionRow>
          <PanelSectionRow>
            <ButtonItem layout="below" onClick={onClear} disabled={clearing}>
              {clearing ? "Clearing..." : "Clear synced games"}
            </ButtonItem>
          </PanelSectionRow>
        </>
      )}
    </PanelSection>
  );
}
