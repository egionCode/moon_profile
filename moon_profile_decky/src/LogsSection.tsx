import { CSSProperties, useEffect, useState } from "react";
import { PanelSection, PanelSectionRow, ButtonItem } from "@decky/ui";
import { getLogs } from "./api";

const LINES = 300;

const logStyle: CSSProperties = {
  whiteSpace: "pre-wrap",
  wordBreak: "break-all",
  fontFamily: "monospace",
  fontSize: "0.7rem",
  maxHeight: "60vh",
  overflowY: "auto",
};

// Aba "Logs" da sidenav de Configuracoes - le decky.DECKY_PLUGIN_LOG (o
// arquivo da sessao atual, ver main.py:get_logs) sem precisar de SSH +
// journalctl. So' busca sob demanda (botao Atualizar), nao fica pollando -
// log e' pra depurar um problema pontual, nao pra ficar de olho o tempo todo.
export function LogsSection() {
  const [logs, setLogs] = useState("");
  const [loading, setLoading] = useState(false);

  const refresh = async () => {
    setLoading(true);
    try {
      setLogs(await getLogs(LINES));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <PanelSection>
      <PanelSectionRow>
        <ButtonItem layout="below" onClick={refresh} disabled={loading}>
          {loading ? "Atualizando..." : "Atualizar"}
        </ButtonItem>
      </PanelSectionRow>
      <PanelSectionRow>
        <div style={logStyle}>{logs || "Carregando..."}</div>
      </PanelSectionRow>
    </PanelSection>
  );
}
