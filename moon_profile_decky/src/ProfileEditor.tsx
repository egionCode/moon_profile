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
// value, as already seen in Phase 0/1).
const RESOLUTION_RE = /^\d+x\d+$/;

// The data is still stored as the string "3840x2160" (it's the format the
// backend/runner/Apollo expect, see main.py and runner.py), only the UI
// splits it into two fields (Width/Height) to make it easier to edit.
function splitResolution(value: string): { width: string; height: string } {
  const [width = "", height = ""] = value.split("x");
  return { width, height };
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
  // The host's real monitors (via the MoonProfile Runner, see
  // moon_profile_runner/src-tauri/src/displays.rs). While empty (still
  // loading, or the Runner is unreachable), the fields below fall back to
  // the old free-text input, so the user isn't stuck unable to edit just
  // because the Runner didn't respond.
  const [displays, setDisplays] = useState<HostDisplay[]>([]);

  useEffect(() => {
    listHostDisplays().then((result) => {
      if (result.ok) {
        setDisplays(result.displays);
      }
    });
  }, []);

  const moonlightRes = splitResolution(draft.moonlight.resolution);
  const hostRes = splitResolution(draft.host.resolution);

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
    if (!RESOLUTION_RE.test(draft.moonlight.resolution)) {
      toaster.toast({ title: "MoonProfile", body: 'Invalid Moonlight resolution (format "3840x2160")' });
      return;
    }
    if (!RESOLUTION_RE.test(draft.host.resolution)) {
      toaster.toast({ title: "MoonProfile", body: 'Invalid Host resolution (format "3840x2160")' });
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

      <PanelSection title="Moonlight (client)">
        <PanelSectionRow>
          <Focusable style={rowStyle}>
            <div style={halfStyle}>
              <TextField
                label="Width"
                mustBeNumeric
                value={moonlightRes.width}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    moonlight: { ...draft.moonlight, resolution: `${e.target.value}x${moonlightRes.height}` },
                  })
                }
              />
            </div>
            <div style={halfStyle}>
              <TextField
                label="Height"
                mustBeNumeric
                value={moonlightRes.height}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    moonlight: { ...draft.moonlight, resolution: `${moonlightRes.width}x${e.target.value}` },
                  })
                }
              />
            </div>
          </Focusable>
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="FPS"
            mustBeNumeric
            value={String(draft.moonlight.fps)}
            onChange={(e) => setDraft({ ...draft, moonlight: { ...draft.moonlight, fps: Number(e.target.value) || 0 } })}
          />
        </PanelSectionRow>
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
          {displays.length > 0 ? (
            <DropdownItem
              label="Target output"
              rgOptions={targetOutputOptions}
              selectedOption={draft.host.target_output}
              onChange={(o) => setDraft({ ...draft, host: { ...draft.host, target_output: o.data } })}
            />
          ) : (
            <TextField
              label="Target output"
              value={draft.host.target_output}
              onChange={(e) => setDraft({ ...draft, host: { ...draft.host, target_output: e.target.value } })}
            />
          )}
        </PanelSectionRow>
        <PanelSectionRow>
          <Focusable style={rowStyle}>
            <div style={halfStyle}>
              <TextField
                label="Width"
                mustBeNumeric
                value={hostRes.width}
                onChange={(e) =>
                  setDraft({ ...draft, host: { ...draft.host, resolution: `${e.target.value}x${hostRes.height}` } })
                }
              />
            </div>
            <div style={halfStyle}>
              <TextField
                label="Height"
                mustBeNumeric
                value={hostRes.height}
                onChange={(e) =>
                  setDraft({ ...draft, host: { ...draft.host, resolution: `${hostRes.width}x${e.target.value}` } })
                }
              />
            </div>
          </Focusable>
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="FPS"
            mustBeNumeric
            value={String(draft.host.fps)}
            onChange={(e) => setDraft({ ...draft, host: { ...draft.host, fps: Number(e.target.value) || 0 } })}
          />
        </PanelSectionRow>
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
