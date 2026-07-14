// Aba "Jogos" da sidenav de Configuracoes - grid mostrando os atalhos por
// jogo ja sincronizados (ver gameSync.ts), com capa quando disponivel (so'
// pra jogos Steam reais por enquanto - Estagio A). Criar atalho continua
// sendo so' pelo botao "Sincronizar jogos do host" no Quick Access; o
// botao "Limpar" aqui remove tudo (da Steam e do arquivo persistido).
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

// Capsula vertical da Steam e' 2:3 (ex: 600x900) - mantem essa proporcao
// mesmo antes da imagem carregar, pra nao pular o layout quando chega.
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
      return; // non-Steam ainda nao tem fonte de capa (Estagio B - SteamGridDB)
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
      toaster.toast({ title: "MoonProfile", body: "Jogos sincronizados removidos" });
    } catch (e) {
      console.error("MoonProfile: erro inesperado limpando jogos sincronizados", e);
      toaster.toast({ title: "MoonProfile - erro inesperado", body: String(e) });
    } finally {
      setClearing(false);
    }
  };

  const entries = Object.entries(shortcuts);

  return (
    <PanelSection>
      {!loaded && <PanelSectionRow>Carregando...</PanelSectionRow>}
      {loaded && entries.length === 0 && (
        <PanelSectionRow>
          Nenhum jogo sincronizado ainda - use &quot;Sincronizar jogos do host&quot; no Quick Access.
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
              {clearing ? "Limpando..." : "Limpar jogos sincronizados"}
            </ButtonItem>
          </PanelSectionRow>
        </>
      )}
    </PanelSection>
  );
}
