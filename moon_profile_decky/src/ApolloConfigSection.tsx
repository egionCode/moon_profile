import { PanelSection, PanelSectionRow, TextField, ButtonItem, DialogBodyText } from "@decky/ui";
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
