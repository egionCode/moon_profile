import { CSSProperties, useEffect, useState } from "react";
import { PanelSection, PanelSectionRow, TextField, DropdownItem, ToggleField, DialogButton, Focusable } from "@decky/ui";
import { toaster } from "@decky/api";
import { listHostDisplays } from "./api";
import { HostDisplay, Profile } from "./types";

// Same pattern as ProfileList.tsx: "ButtonItem"/"TextField" occupy the
// whole row by themselves, which is why two side by side (Cancel/Save,
// Width/Height) would stack instead of splitting the line. A Focusable
// with display:flex, with each child wrapped in a div with flexGrow:1,
// solves both cases.
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

// Sets the SAME resolution/fps on both host and moonlight at once -
// there's exactly one "Resolution" concept in the UI now (see the
// "Display" section below): whatever the host output is switched to is
// also what Moonlight streams at, no reason for the two to ever differ.
function applyResolution(draft: Profile, resolution: string, fps: number): Profile {
  return {
    ...draft,
    moonlight: { ...draft.moonlight, resolution, fps },
    host: { ...draft.host, resolution, fps },
  };
}

interface ProfileEditorProps {
  profile: Profile;
  isNew: boolean;
  existingIds: string[];
  onSave: (profile: Profile) => void;
  onCancel: () => void;
}

export function ProfileEditor({ profile, isNew, existingIds, onSave, onCancel }: ProfileEditorProps) {
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

  const manualRes = splitResolution(draft.host.resolution);
  const selectedDisplay = displays.find((d) => d.name === draft.host.target_output);
  const modeOptions = (selectedDisplay?.modes ?? []).map((m) => ({
    data: `${m.resolution}@${m.fps}`,
    label: `${m.resolution} @ ${m.fps} FPS`,
  }));
  const selectedModeKey = `${draft.host.resolution}@${draft.host.fps}`;

  const targetOutputOptions = displays.map((d) => ({
    data: d.name,
    label: d.connected ? d.name : `${d.name} (disconnected)`,
  }));

  const onSubmit = () => {
    if (!draft.name.trim()) {
      toaster.toast({ title: "MoonProfile", body: "Profile name cannot be empty" });
      return;
    }
    if (!draft.id.trim()) {
      toaster.toast({ title: "MoonProfile", body: "Profile id cannot be empty" });
      return;
    }
    if (isNew && existingIds.includes(draft.id)) {
      toaster.toast({ title: "MoonProfile", body: `A profile with id "${draft.id}" already exists` });
      return;
    }
    if (!RESOLUTION_RE.test(draft.host.resolution)) {
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
          <TextField label="Id" disabled={!isNew} value={draft.id} onChange={(e) => setDraft({ ...draft, id: e.target.value })} />
        </PanelSectionRow>
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
                label="Resolution"
                rgOptions={modeOptions}
                selectedOption={selectedModeKey}
                onChange={(o) => {
                  const [resolution, fps] = o.data.split("@");
                  setDraft((prev) => applyResolution(prev, resolution, Number(fps)));
                }}
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
                    onChange={(e) => setDraft(applyResolution(draft, `${e.target.value}x${manualRes.height}`, draft.host.fps))}
                  />
                </div>
                <div style={halfStyle}>
                  <TextField
                    label="Height"
                    mustBeNumeric
                    value={manualRes.height}
                    onChange={(e) => setDraft(applyResolution(draft, `${manualRes.width}x${e.target.value}`, draft.host.fps))}
                  />
                </div>
              </Focusable>
            </PanelSectionRow>
            <PanelSectionRow>
              <TextField
                label="FPS"
                mustBeNumeric
                value={String(draft.host.fps)}
                onChange={(e) => setDraft(applyResolution(draft, draft.host.resolution, Number(e.target.value) || 0))}
              />
            </PanelSectionRow>
          </>
        )}
      </PanelSection>

      <PanelSection title="Moonlight (client)">
        <PanelSectionRow>
          <TextField
            label="Bitrate (kbps)"
            mustBeNumeric
            value={String(draft.moonlight.bitrate)}
            onChange={(e) => setDraft({ ...draft, moonlight: { ...draft.moonlight, bitrate: Number(e.target.value) || 0 } })}
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
        <PanelSectionRow>
          <ToggleField
            label="HDR"
            checked={draft.moonlight.hdr}
            onChange={(checked) => setDraft({ ...draft, moonlight: { ...draft.moonlight, hdr: checked } })}
          />
        </PanelSectionRow>
      </PanelSection>

      <PanelSection title="Host (Apollo)">
        <PanelSectionRow>
          <ToggleField
            label="HDR"
            checked={draft.host.hdr}
            onChange={(checked) => setDraft({ ...draft, host: { ...draft.host, hdr: checked } })}
          />
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
