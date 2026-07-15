import { PanelSection, PanelSectionRow, TextField, ButtonItem, DialogBodyText } from "@decky/ui";
import { Config } from "./types";

interface RunnerConfigSectionProps {
  config: Config;
  setConfig: (config: Config) => void;
  onSave: () => void;
}

// "Runner" tab of the Settings sidenav: the MoonProfile Runner is the
// Tauri/Rust daemon running on the host (PRD Phase 5). It's no longer
// optional: Apollo has no prep-cmd at all (neither do nor undo), it's the
// Runner that turns on the display at launch and turns it off at close,
// besides detecting on its own when the game closes internally. Only the
// port is configurable here, the host is always the same one from the
// "Apollo Config" tab (Runner and Apollo run on the same machine, asking
// for the IP twice would be redundant and confusing).
export function RunnerConfigSection({ config, setConfig, onSave }: RunnerConfigSectionProps) {
  return (
    <>
      <PanelSection>
        <PanelSectionRow>
          <DialogBodyText>
            Required. The MoonProfile Runner needs to be installed and running on the host to
            switch the display (resolution/monitor) on launch and disconnect, and to
            automatically detect when the game closes internally. Uses the same host configured
            in the "Apollo Config" tab. A local server open on the network, without authentication.
          </DialogBodyText>
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Port"
            mustBeNumeric
            value={String(config.runner_port)}
            onChange={(e) => setConfig({ ...config, runner_port: Number(e.target.value) || 0 })}
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
