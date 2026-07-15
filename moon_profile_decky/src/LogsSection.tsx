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

// "Logs" tab of the Settings sidenav: reads decky.DECKY_PLUGIN_LOG (the
// current session's file, see main.py:get_logs) without needing SSH +
// journalctl. Only fetches on demand (Refresh button), doesn't poll, logs
// are for debugging a specific problem, not for watching continuously.
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
          {loading ? "Refreshing..." : "Refresh"}
        </ButtonItem>
      </PanelSectionRow>
      <PanelSectionRow>
        <div style={logStyle}>{logs || "Loading..."}</div>
      </PanelSectionRow>
    </PanelSection>
  );
}
