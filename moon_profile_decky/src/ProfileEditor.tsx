import { CSSProperties, useEffect, useState } from "react";
import {
  PanelSection,
  PanelSectionRow,
  TextField,
  DropdownItem,
  ToggleField,
  SliderField,
  DialogButton,
  Focusable,
} from "@decky/ui";
import { toaster } from "@decky/api";
import { listHostDisplays } from "./api";
import { HostDisplay, HostDisplayMode, MoonlightConfig, Profile } from "./types";

// Same pattern as ProfileList.tsx: "ButtonItem"/"TextField" occupy the
// whole row by themselves, which is why two side by side (Cancel/Save,
// Width/Height) would stack instead of splitting the line. A Focusable
// with display:flex, with each child wrapped in a div with flexGrow:1,
// solves that.
const rowStyle: CSSProperties = { display: "flex", flexDirection: "row", gap: "8px" };
const halfStyle: CSSProperties = { flexGrow: 1, minWidth: 0 };

const TRIGGER_OPTIONS = [
  { data: "docked", label: "Docked" },
  { data: "handheld", label: "Handheld" },
  { data: "manual", label: "Manual" },
];

const CODEC_OPTIONS = [
  { data: "HEVC", label: "HEVC" },
  { data: "AV1", label: "AV1" },
  { data: "H264", label: "H264" },
];

const DISPLAY_MODE_OPTIONS = [
  { data: "fullscreen", label: "Fullscreen" },
  { data: "windowed", label: "Windowed" },
  { data: "borderless", label: "Borderless" },
];

const VIDEO_DECODER_OPTIONS = [
  { data: "auto", label: "Auto" },
  { data: "hardware", label: "Hardware" },
  { data: "software", label: "Software" },
];

const CAPTURE_SYSTEM_KEYS_OPTIONS = [
  { data: "never", label: "Never" },
  { data: "fullscreen", label: "Fullscreen only" },
  { data: "always", label: "Always" },
];

const AUDIO_CONFIG_OPTIONS = [
  { data: "stereo", label: "Stereo" },
  { data: "5.1-surround", label: "5.1 surround" },
  { data: "7.1-surround", label: "7.1 surround" },
];

// ex: "3840x2160" - basic validation, just to catch typos before sending
// it to Apollo/Moonlight (which fail in confusing ways with an invalid
// value, as already seen in Phase 0/1). Only reachable via the manual
// fallback fields below (the real Monitor/Resolution selects always
// produce a well-formed value straight from the Runner).
const RESOLUTION_RE = /^\d+x\d+$/;

// The data is still stored as the string "3840x2160" (it's the format the
// backend/runner/Apollo expect, see main.py and runner.py), only the UI
// splits it into two fields (Width/Height) for the manual fallback.
function splitResolution(value: string): { width: string; height: string } {
  const [width = "", height = ""] = value.split("x");
  return { width, height };
}

// Sets the SAME resolution/fps at the Profile root - there's exactly one
// "Resolution" concept now (used to be duplicated under host/moonlight,
// see git history): whatever the host output is switched to is also what
// Moonlight streams at, no reason for the two to ever differ.
function applyResolution(draft: Profile, resolution: string, fps: number): Profile {
  return { ...draft, resolution, fps };
}

function gcd(a: number, b: number): number {
  return b === 0 ? a : gcd(b, a % b);
}

// Common display aspect ratios - anything close enough to one of these
// (a portrait/ultrawide monitor doesn't map to any) snaps to its label;
// otherwise falls back to the raw reduced ratio (ex: "21:9") so nothing
// is silently misrepresented.
const KNOWN_RATIOS: [string, number][] = [
  ["4:3", 4 / 3],
  ["16:10", 16 / 10],
  ["16:9", 16 / 9],
];

function aspectRatioLabel(resolution: string): string {
  const [wStr, hStr] = resolution.split("x");
  const width = Number(wStr);
  const height = Number(hStr);
  if (!width || !height) {
    return "?";
  }
  const ratio = width / height;
  let best = KNOWN_RATIOS[0];
  let bestDiff = Infinity;
  for (const candidate of KNOWN_RATIOS) {
    const diff = Math.abs(ratio - candidate[1]);
    if (diff < bestDiff) {
      bestDiff = diff;
      best = candidate;
    }
  }
  if (bestDiff < 0.02) {
    return best[0];
  }
  const divisor = gcd(width, height);
  return `${width / divisor}:${height / divisor}`;
}

// Groups a display's real modes (as reported by the Runner/kscreen-doctor)
// by aspect ratio, so the "Aspect ratio" / "Resolution" / "FPS" selects
// only ever offer combinations the monitor actually supports - no static
// table up to some resolution ceiling, purely derived from HostDisplay.modes.
function groupModesByAspectRatio(modes: HostDisplayMode[]) {
  const resolutionsByRatio = new Map<string, string[]>();
  const fpsByResolution = new Map<string, number[]>();
  for (const mode of modes) {
    const ratio = aspectRatioLabel(mode.resolution);
    const resolutions = resolutionsByRatio.get(ratio) ?? [];
    if (!resolutions.includes(mode.resolution)) {
      resolutions.push(mode.resolution);
    }
    resolutionsByRatio.set(ratio, resolutions);

    const fpsList = fpsByResolution.get(mode.resolution) ?? [];
    if (!fpsList.includes(mode.fps)) {
      fpsList.push(mode.fps);
    }
    fpsByResolution.set(mode.resolution, fpsList);
  }
  return { ratios: [...resolutionsByRatio.keys()], resolutionsByRatio, fpsByResolution };
}

interface ProfileEditorProps {
  profile: Profile;
  isNew: boolean;
  onSave: (profile: Profile) => void;
  onCancel: () => void;
}

export function ProfileEditor({ profile, isNew, onSave, onCancel }: ProfileEditorProps) {
  const [draft, setDraft] = useState<Profile>(profile);
  const [disableOutputsText, setDisableOutputsText] = useState(draft.host.disable_outputs.join(", "));
  // The host's real monitors + their supported resolutions/fps (via the
  // MoonProfile Runner, see moon_profile_runner/src-tauri/src/displays.rs).
  // loadingDisplays distinguishes "still asking the Runner" from "asked,
  // got nothing back" (Runner unreachable, or genuinely no monitors) -
  // the manual fallback fields below only show up once we know it's the
  // latter, so the user isn't stuck unable to edit while the request is
  // still in flight, or has no data to go on at all.
  const [displays, setDisplays] = useState<HostDisplay[]>([]);
  const [loadingDisplays, setLoadingDisplays] = useState(true);

  useEffect(() => {
    listHostDisplays()
      .then((result) => {
        if (result.ok) {
          setDisplays(result.displays);
        }
      })
      .finally(() => setLoadingDisplays(false));
  }, []);

  const manualRes = splitResolution(draft.resolution);
  const selectedDisplay = displays.find((d) => d.name === draft.host.target_output);
  const modeGroups = groupModesByAspectRatio(selectedDisplay?.modes ?? []);
  const currentRatio = aspectRatioLabel(draft.resolution);
  const ratioOptions = modeGroups.ratios.map((r) => ({ data: r, label: r }));
  const resolutionOptions = (modeGroups.resolutionsByRatio.get(currentRatio) ?? []).map((r) => ({
    data: r,
    label: r,
  }));
  const fpsOptions = (modeGroups.fpsByResolution.get(draft.resolution) ?? []).map((f) => ({
    data: f,
    label: `${f} FPS`,
  }));

  const targetOutputOptions = displays.map((d) => ({
    data: d.name,
    label: d.connected ? d.name : `${d.name} (disconnected)`,
  }));

  // Shared by every field in "Advanced video"/"Input"/"Audio" below - all
  // of them just patch one key of draft.moonlight, only the value type
  // (boolean vs a choice's string) changes.
  function setMoonlight<K extends keyof MoonlightConfig>(key: K, value: MoonlightConfig[K]) {
    setDraft((prev) => ({ ...prev, moonlight: { ...prev.moonlight, [key]: value } }));
  }

  const onSubmit = () => {
    if (!draft.name.trim()) {
      toaster.toast({ title: "MoonProfile", body: "Profile name cannot be empty" });
      return;
    }
    if (!RESOLUTION_RE.test(draft.resolution)) {
      toaster.toast({ title: "MoonProfile", body: 'Invalid resolution (format "3840x2160")' });
      return;
    }

    // With the real monitor list (displays.length > 0), each output's
    // toggle already keeps draft.host.disable_outputs up to date, only
    // need to parse the free text in the fallback (Runner unreachable).
    const disable_outputs =
      displays.length > 0
        ? draft.host.disable_outputs
        : disableOutputsText
            .split(",")
            .map((s) => s.trim())
            .filter((s) => s.length > 0);

    onSave({ ...draft, host: { ...draft.host, disable_outputs } });
  };

  return (
    <>
      <PanelSection title={isNew ? "New profile" : `Edit: ${profile.name}`}>
        <PanelSectionRow>
          <TextField label="Name" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
        </PanelSectionRow>
        <PanelSectionRow>
          <DropdownItem
            label="Trigger"
            rgOptions={TRIGGER_OPTIONS}
            selectedOption={draft.trigger}
            onChange={(o) => setDraft({ ...draft, trigger: o.data })}
          />
        </PanelSectionRow>
      </PanelSection>

      <PanelSection title="Display">
        {loadingDisplays ? (
          <PanelSectionRow>Loading available monitors...</PanelSectionRow>
        ) : displays.length > 0 ? (
          <>
            <PanelSectionRow>
              <DropdownItem
                label="Monitor"
                rgOptions={targetOutputOptions}
                selectedOption={draft.host.target_output}
                onChange={(o) => {
                  const display = displays.find((d) => d.name === o.data);
                  const firstMode = display?.modes[0];
                  setDraft((prev) => {
                    const withOutput = { ...prev, host: { ...prev.host, target_output: o.data } };
                    return firstMode ? applyResolution(withOutput, firstMode.resolution, firstMode.fps) : withOutput;
                  });
                }}
              />
            </PanelSectionRow>
            <PanelSectionRow>
              <DropdownItem
                label="Aspect ratio"
                rgOptions={ratioOptions}
                selectedOption={currentRatio}
                onChange={(o) => {
                  const resolutions = modeGroups.resolutionsByRatio.get(o.data) ?? [];
                  const resolution = resolutions[0];
                  const fps = (modeGroups.fpsByResolution.get(resolution) ?? [draft.fps])[0];
                  if (resolution) {
                    setDraft((prev) => applyResolution(prev, resolution, fps));
                  }
                }}
              />
            </PanelSectionRow>
            <PanelSectionRow>
              <DropdownItem
                label="Resolution"
                rgOptions={resolutionOptions}
                selectedOption={draft.resolution}
                onChange={(o) => {
                  const fps = (modeGroups.fpsByResolution.get(o.data) ?? [draft.fps])[0];
                  setDraft((prev) => applyResolution(prev, o.data, fps));
                }}
              />
            </PanelSectionRow>
            <PanelSectionRow>
              <DropdownItem
                label="FPS"
                rgOptions={fpsOptions}
                selectedOption={draft.fps}
                onChange={(o) => setDraft((prev) => applyResolution(prev, prev.resolution, o.data))}
              />
            </PanelSectionRow>
          </>
        ) : (
          <>
            <PanelSectionRow>
              <TextField
                label="Monitor (target output)"
                value={draft.host.target_output}
                onChange={(e) => setDraft({ ...draft, host: { ...draft.host, target_output: e.target.value } })}
              />
            </PanelSectionRow>
            <PanelSectionRow>
              <Focusable style={rowStyle}>
                <div style={halfStyle}>
                  <TextField
                    label="Width"
                    mustBeNumeric
                    value={manualRes.width}
                    onChange={(e) => setDraft(applyResolution(draft, `${e.target.value}x${manualRes.height}`, draft.fps))}
                  />
                </div>
                <div style={halfStyle}>
                  <TextField
                    label="Height"
                    mustBeNumeric
                    value={manualRes.height}
                    onChange={(e) => setDraft(applyResolution(draft, `${manualRes.width}x${e.target.value}`, draft.fps))}
                  />
                </div>
              </Focusable>
            </PanelSectionRow>
            <PanelSectionRow>
              <TextField
                label="FPS"
                mustBeNumeric
                value={String(draft.fps)}
                onChange={(e) => setDraft(applyResolution(draft, draft.resolution, Number(e.target.value) || 0))}
              />
            </PanelSectionRow>
          </>
        )}
        <PanelSectionRow>
          <ToggleField label="HDR" checked={draft.hdr} onChange={(checked) => setDraft({ ...draft, hdr: checked })} />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="WCG"
            checked={draft.host.wcg}
            onChange={(checked) => setDraft({ ...draft, host: { ...draft.host, wcg: checked } })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Enter Big Picture on launch"
            checked={draft.host.enter_bigpicture ?? false}
            onChange={(checked) => setDraft({ ...draft, host: { ...draft.host, enter_bigpicture: checked } })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Move cursor to the corner while playing"
            checked={draft.host.move_cursor_to_corner ?? false}
            onChange={(checked) => setDraft({ ...draft, host: { ...draft.host, move_cursor_to_corner: checked } })}
          />
        </PanelSectionRow>
        {displays.length > 0 ? (
          // Dynamic list, one toggle per real host monitor (except the one
          // already chosen as the target output, doesn't make sense to
          // disable the same one that was just turned on).
          displays
            .filter((d) => d.name !== draft.host.target_output)
            .map((d) => (
              <PanelSectionRow key={d.name}>
                <ToggleField
                  label={`Disable ${d.name}${d.connected ? "" : " (disconnected)"}`}
                  checked={draft.host.disable_outputs.includes(d.name)}
                  onChange={(checked) => {
                    const disable_outputs = checked
                      ? [...draft.host.disable_outputs, d.name]
                      : draft.host.disable_outputs.filter((o) => o !== d.name);
                    setDraft({ ...draft, host: { ...draft.host, disable_outputs } });
                  }}
                />
              </PanelSectionRow>
            ))
        ) : (
          <PanelSectionRow>
            <TextField
              label="Outputs to disable (comma-separated)"
              value={disableOutputsText}
              onChange={(e) => setDisableOutputsText(e.target.value)}
            />
          </PanelSectionRow>
        )}
      </PanelSection>

      <PanelSection title="Streaming quality">
        <PanelSectionRow>
          <SliderField
            // showValue's auto label renders the slider's raw position as
            // a "%" of the min/max range, not our unit - baking the actual
            // Mbps value into the label itself instead avoids that.
            label={`Bitrate: ${Math.round(draft.moonlight.bitrate / 1000)} Mbps`}
            editableValue
            value={draft.moonlight.bitrate / 1000}
            min={1}
            max={300}
            onChange={(mbps) =>
              setDraft({ ...draft, moonlight: { ...draft.moonlight, bitrate: Math.round(mbps * 1000) } })
            }
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <DropdownItem
            label="Codec"
            rgOptions={CODEC_OPTIONS}
            selectedOption={draft.moonlight.codec}
            onChange={(o) => setDraft({ ...draft, moonlight: { ...draft.moonlight, codec: o.data } })}
          />
        </PanelSectionRow>
      </PanelSection>

      <PanelSection title="Advanced video">
        <PanelSectionRow>
          <ToggleField
            label="V-Sync"
            checked={draft.moonlight.vsync ?? true}
            onChange={(checked) => setMoonlight("vsync", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            // Just a bool in moonlight-qt (Q_PROPERTY(bool framePacing...)),
            // no "latency/smoothness" modes - but it only has any effect
            // with V-Sync on: session.cpp gates it as
            // "enableVsync && m_Preferences->framePacing" before it ever
            // reaches the decoder, same as the official GUI graying out
            // this exact checkbox when V-Sync is off (SettingsView.qml).
            label="Frame pacing (requires V-Sync)"
            checked={(draft.moonlight.vsync ?? true) && (draft.moonlight.frame_pacing ?? false)}
            disabled={!(draft.moonlight.vsync ?? true)}
            onChange={(checked) => setMoonlight("frame_pacing", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <DropdownItem
            label="Display mode"
            rgOptions={DISPLAY_MODE_OPTIONS}
            selectedOption={draft.moonlight.display_mode ?? "fullscreen"}
            onChange={(o) => setMoonlight("display_mode", o.data)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <DropdownItem
            label="Video decoder"
            rgOptions={VIDEO_DECODER_OPTIONS}
            selectedOption={draft.moonlight.video_decoder ?? "auto"}
            onChange={(o) => setMoonlight("video_decoder", o.data)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="YUV 4:4:4 (if supported)"
            checked={draft.moonlight.yuv444 ?? false}
            onChange={(checked) => setMoonlight("yuv444", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Performance overlay"
            checked={draft.moonlight.performance_overlay ?? false}
            onChange={(checked) => setMoonlight("performance_overlay", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Keep display awake while streaming"
            checked={draft.moonlight.keep_awake ?? true}
            onChange={(checked) => setMoonlight("keep_awake", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Game optimizations"
            checked={draft.moonlight.game_optimization ?? true}
            onChange={(checked) => setMoonlight("game_optimization", checked)}
          />
        </PanelSectionRow>
      </PanelSection>

      <PanelSection title="Input">
        <PanelSectionRow>
          <ToggleField
            label="Absolute mouse mode"
            checked={draft.moonlight.absolute_mouse ?? false}
            onChange={(checked) => setMoonlight("absolute_mouse", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Swap left/right mouse buttons"
            checked={draft.moonlight.mouse_buttons_swap ?? false}
            onChange={(checked) => setMoonlight("mouse_buttons_swap", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Touchscreen as trackpad"
            checked={draft.moonlight.touchscreen_trackpad ?? false}
            onChange={(checked) => setMoonlight("touchscreen_trackpad", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Multiple controller support"
            checked={draft.moonlight.multi_controller ?? true}
            onChange={(checked) => setMoonlight("multi_controller", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Background gamepad input"
            checked={draft.moonlight.background_gamepad ?? false}
            onChange={(checked) => setMoonlight("background_gamepad", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Reverse scroll direction"
            checked={draft.moonlight.reverse_scroll_direction ?? false}
            onChange={(checked) => setMoonlight("reverse_scroll_direction", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Swap A/B and X/Y (Nintendo-style)"
            checked={draft.moonlight.swap_gamepad_buttons ?? false}
            onChange={(checked) => setMoonlight("swap_gamepad_buttons", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <DropdownItem
            label="Capture system key combos"
            rgOptions={CAPTURE_SYSTEM_KEYS_OPTIONS}
            selectedOption={draft.moonlight.capture_system_keys ?? "never"}
            onChange={(o) => setMoonlight("capture_system_keys", o.data)}
          />
        </PanelSectionRow>
      </PanelSection>

      <PanelSection title="Audio">
        <PanelSectionRow>
          <DropdownItem
            label="Audio config"
            rgOptions={AUDIO_CONFIG_OPTIONS}
            selectedOption={draft.moonlight.audio_config ?? "stereo"}
            onChange={(o) => setMoonlight("audio_config", o.data)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Play audio on host"
            checked={draft.moonlight.audio_on_host ?? false}
            onChange={(checked) => setMoonlight("audio_on_host", checked)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Mute when window loses focus"
            checked={draft.moonlight.mute_on_focus_loss ?? false}
            onChange={(checked) => setMoonlight("mute_on_focus_loss", checked)}
          />
        </PanelSectionRow>
      </PanelSection>

      <PanelSection>
        <PanelSectionRow>
          <Focusable style={rowStyle}>
            <DialogButton style={halfStyle} onClick={onCancel}>
              Cancel
            </DialogButton>
            <DialogButton style={halfStyle} onClick={onSubmit}>
              Save profile
            </DialogButton>
          </Focusable>
        </PanelSectionRow>
      </PanelSection>
    </>
  );
}
