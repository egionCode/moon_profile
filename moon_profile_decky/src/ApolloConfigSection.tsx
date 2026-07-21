import { useState } from "react";
import { PanelSection, PanelSectionRow, TextField, ButtonItem, DialogBodyText, Field } from "@decky/ui";
import { toaster } from "@decky/api";
import { fetchHostMac } from "./api";
import { Config } from "./types";

interface ApolloConfigSectionProps {
  config: Config;
  setConfig: (config: Config) => void;
  onSave: () => void;
}

// "Apollo Config" tab of the Settings sidenav (SettingsPage.tsx). Just
// host/username/password, with its own Save. The ("config") state is owned
// by SettingsPage, not here, so switching tabs without saving does not lose
// edits made on the button-positioning tab (both work on the SAME Config
// object, which is saved whole in one go on the backend).
export function ApolloConfigSection({ config, setConfig, onSave }: ApolloConfigSectionProps) {
  const [detecting, setDetecting] = useState(false);

  // Requires the host to already be reachable (asks the Runner's GET
  // /system/mac), which is why this lives here and not in Quick Access -
  // fetch_host_mac (main.py) already persists the result on the backend,
  // this just mirrors it into the locally-edited config so it isn't lost
  // if the user then edits something else and clicks Save.
  const onDetectMac = async () => {
    setDetecting(true);
    try {
      const result = await fetchHostMac();
      if (result.ok && result.mac) {
        setConfig({ ...config, mac_address: result.mac });
        toaster.toast({ title: "MoonProfile", body: `MAC detected: ${result.mac}` });
      } else {
        toaster.toast({ title: "MoonProfile - error", body: result.error ?? "Unknown failure" });
      }
    } catch (e) {
      console.error("MoonProfile: unexpected error detecting the host MAC", e);
      toaster.toast({ title: "MoonProfile - unexpected error", body: String(e) });
    } finally {
      setDetecting(false);
    }
  };

  return (
    <>
      <PanelSection>
        <PanelSectionRow>
          <DialogBodyText>
            These are the same admin credentials you use to sign in to the Apollo web panel
            (the streaming server running on your PC/host). The plugin uses the Apollo API
            with them to configure the host display and start streaming sessions automatically.
            Nothing is sent outside your local network.
          </DialogBodyText>
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Host"
            value={config.host}
            onChange={(e) => setConfig({ ...config, host: e.target.value })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Username"
            value={config.username}
            onChange={(e) => setConfig({ ...config, username: e.target.value })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Password"
            bIsPassword
            value={config.password}
            onChange={(e) => setConfig({ ...config, password: e.target.value })}
          />
        </PanelSectionRow>
      </PanelSection>

      <PanelSection title="Wake-on-LAN">
        <PanelSectionRow>
          <Field label="MAC address">{config.mac_address || "Not detected yet"}</Field>
        </PanelSectionRow>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onDetectMac} disabled={detecting}>
            {detecting ? "Detecting..." : "Detect MAC from host"}
          </ButtonItem>
        </PanelSectionRow>
      </PanelSection>

      <PanelSection>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onSave}>
            Save
          </ButtonItem>
        </PanelSectionRow>
      </PanelSection>
    </>
  );
}
