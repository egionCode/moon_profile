import { CSSProperties, useEffect, useState } from "react";
import { PanelSection, PanelSectionRow, TextField, DropdownItem, ToggleField, DialogButton, Focusable } from "@decky/ui";
import { toaster } from "@decky/api";
import { listHostDisplays } from "./api";
import { HostDisplay, Profile } from "./types";

// Mesmo padrao do ProfileList.tsx: "ButtonItem"/"TextField" ocupam a row
// inteira sozinhos, por isso dois lado a lado (Cancelar/Salvar, Largura/
// Altura) empilhavam em vez de dividir a linha. Um Focusable com
// display:flex, com cada filho envolto num div flexGrow:1, resolve pros
// dois casos.
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

// ex: "3840x2160" - validacao basica, so pra pegar erro de digitacao antes
// de mandar pro Apollo/Moonlight (que falham de formas confusas com um
// valor invalido, como ja vimos na Fase 0/1).
const RESOLUTION_RE = /^\d+x\d+$/;

// O dado continua guardado como string "3840x2160" (e' o formato que o
// backend/runner/Apollo esperam - ver main.py e runner.py), so' a UI que
// separa em dois campos (Largura/Altura) pra ficar mais facil de editar.
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
  // Monitores de verdade do host (via MoonProfile Runner - ver
  // moon_profile_runner/src-tauri/src/displays.rs). Enquanto vazio (ainda
  // carregando, ou o Runner esta' inalcancavel), os campos abaixo caem pro
  // texto livre de antes - nao deixa o usuario travado sem poder editar
  // so' porque o Runner nao respondeu.
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
    label: d.connected ? d.name : `${d.name} (desconectado)`,
  }));

  const onSubmit = () => {
    if (!draft.name.trim()) {
      toaster.toast({ title: "MoonProfile", body: "Nome do perfil nao pode ser vazio" });
      return;
    }
    if (!draft.id.trim()) {
      toaster.toast({ title: "MoonProfile", body: "Id do perfil nao pode ser vazio" });
      return;
    }
    if (isNew && existingIds.includes(draft.id)) {
      toaster.toast({ title: "MoonProfile", body: `Ja existe um perfil com id "${draft.id}"` });
      return;
    }
    if (!RESOLUTION_RE.test(draft.moonlight.resolution)) {
      toaster.toast({ title: "MoonProfile", body: 'Resolucao do Moonlight invalida (formato "3840x2160")' });
      return;
    }
    if (!RESOLUTION_RE.test(draft.host.resolution)) {
      toaster.toast({ title: "MoonProfile", body: 'Resolucao do Host invalida (formato "3840x2160")' });
      return;
    }

    // Com a lista de monitores de verdade (displays.length > 0), o toggle
    // de cada output ja mantem draft.host.disable_outputs atualizado -
    // so' precisa parsear o texto livre no fallback (Runner inalcancavel).
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
      <PanelSection title={isNew ? "Novo perfil" : `Editar: ${profile.name}`}>
        <PanelSectionRow>
          <TextField label="Id" disabled={!isNew} value={draft.id} onChange={(e) => setDraft({ ...draft, id: e.target.value })} />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField label="Nome" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
        </PanelSectionRow>
        <PanelSectionRow>
          <DropdownItem
            label="Gatilho"
            rgOptions={TRIGGER_OPTIONS}
            selectedOption={draft.trigger}
            onChange={(o) => setDraft({ ...draft, trigger: o.data })}
          />
        </PanelSectionRow>
      </PanelSection>

      <PanelSection title="Moonlight (cliente)">
        <PanelSectionRow>
          <Focusable style={rowStyle}>
            <div style={halfStyle}>
              <TextField
                label="Largura"
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
                label="Altura"
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
              label="Output alvo"
              rgOptions={targetOutputOptions}
              selectedOption={draft.host.target_output}
              onChange={(o) => setDraft({ ...draft, host: { ...draft.host, target_output: o.data } })}
            />
          ) : (
            <TextField
              label="Output alvo"
              value={draft.host.target_output}
              onChange={(e) => setDraft({ ...draft, host: { ...draft.host, target_output: e.target.value } })}
            />
          )}
        </PanelSectionRow>
        <PanelSectionRow>
          <Focusable style={rowStyle}>
            <div style={halfStyle}>
              <TextField
                label="Largura"
                mustBeNumeric
                value={hostRes.width}
                onChange={(e) =>
                  setDraft({ ...draft, host: { ...draft.host, resolution: `${e.target.value}x${hostRes.height}` } })
                }
              />
            </div>
            <div style={halfStyle}>
              <TextField
                label="Altura"
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
        {displays.length > 0 ? (
          // Lista dinamica - um toggle por monitor de verdade do host
          // (menos o que ja' esta' escolhido como output alvo, nao faz
          // sentido desabilitar o mesmo que acabou de ser ligado).
          displays
            .filter((d) => d.name !== draft.host.target_output)
            .map((d) => (
              <PanelSectionRow key={d.name}>
                <ToggleField
                  label={`Desabilitar ${d.name}${d.connected ? "" : " (desconectado)"}`}
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
              label="Outputs a desabilitar (separados por virgula)"
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
              Cancelar
            </DialogButton>
            <DialogButton style={halfStyle} onClick={onSubmit}>
              Salvar perfil
            </DialogButton>
          </Focusable>
        </PanelSectionRow>
      </PanelSection>
    </>
  );
}
