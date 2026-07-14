import { CSSProperties } from "react";
import { PanelSection, PanelSectionRow, TextField, ButtonItem, DropdownItem, Focusable } from "@decky/ui";
import { ButtonPosition, Config } from "./types";

// Predefinicoes rapidas pro select abaixo - preenchem os 4 campos de ajuste
// fino (Top/Bottom/Left/Right) com um ponto de partida razoavel; o usuario
// pode continuar ajustando os campos na mao depois (por isso "center" so
// usa "left: 50%" sem transform de recentralizacao - e' so' um chute
// inicial, nao precisa ser pixel-perfect).
const POSITION_PRESETS: Record<string, ButtonPosition> = {
  "bottom-left": { top: "", bottom: "2.8vw", left: "32px", right: "" },
  "bottom-right": { top: "", bottom: "2.8vw", left: "", right: "32px" },
  "bottom-center": { top: "", bottom: "2.8vw", left: "50%", right: "" },
  "top-left": { top: "2.8vw", bottom: "", left: "32px", right: "" },
  "top-right": { top: "2.8vw", bottom: "", left: "", right: "32px" },
  "top-center": { top: "2.8vw", bottom: "", left: "50%", right: "" },
};

const CUSTOM_PRESET = "custom";

const PRESET_OPTIONS = [
  { data: "bottom-left", label: "Inferior esquerdo" },
  { data: "bottom-right", label: "Inferior direito" },
  { data: "bottom-center", label: "Inferior centro" },
  { data: "top-left", label: "Superior esquerdo" },
  { data: "top-right", label: "Superior direito" },
  { data: "top-center", label: "Superior centro" },
  { data: CUSTOM_PRESET, label: "Personalizado (ajuste fino)" },
];

// Deriva qual predefinicao "bate" com a posicao atual, em vez de guardar o
// preset selecionado como state a parte - senao o select fica preso no
// valor inicial e nunca reflete o que ja' foi salvo (bug ja visto aqui).
function findMatchingPreset(position: ButtonPosition): string {
  const match = Object.entries(POSITION_PRESETS).find(
    ([, preset]) =>
      preset.top === position.top &&
      preset.bottom === position.bottom &&
      preset.left === position.left &&
      preset.right === position.right,
  );
  return match ? match[0] : CUSTOM_PRESET;
}

const rowStyle: CSSProperties = { display: "flex", flexDirection: "row", gap: "8px" };
const halfStyle: CSSProperties = { flexGrow: 1, minWidth: 0 };

interface ButtonPositionSectionProps {
  config: Config;
  setConfig: (config: Config) => void;
  onSave: () => void;
}

// Aba "Posicao do botao" da sidenav de Configuracoes (SettingsPage.tsx) -
// mesmo padrao de dono-do-estado-e-o-pai que ApolloConfigSection.tsx.
export function ButtonPositionSection({ config, setConfig, onSave }: ButtonPositionSectionProps) {
  const selectedPreset = findMatchingPreset(config.button_position);

  const onPresetChange = (preset: string) => {
    if (preset === CUSTOM_PRESET) {
      return; // "Personalizado" e' so' o estado de "nao bate com nenhum preset" - nao ha' valores pra aplicar
    }
    setConfig({ ...config, button_position: POSITION_PRESETS[preset] });
  };

  const setPositionField = (field: keyof ButtonPosition, value: string) => {
    setConfig({ ...config, button_position: { ...config.button_position, [field]: value } });
  };

  return (
    <>
      <PanelSection>
        <PanelSectionRow>
          <DropdownItem
            label="Predefinicao"
            rgOptions={PRESET_OPTIONS}
            selectedOption={selectedPreset}
            onChange={(o) => onPresetChange(o.data)}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <Focusable style={rowStyle}>
            <div style={halfStyle}>
              <TextField
                label="Top"
                value={config.button_position.top}
                onChange={(e) => setPositionField("top", e.target.value)}
              />
            </div>
            <div style={halfStyle}>
              <TextField
                label="Bottom"
                value={config.button_position.bottom}
                onChange={(e) => setPositionField("bottom", e.target.value)}
              />
            </div>
          </Focusable>
        </PanelSectionRow>
        <PanelSectionRow>
          <Focusable style={rowStyle}>
            <div style={halfStyle}>
              <TextField
                label="Left"
                value={config.button_position.left}
                onChange={(e) => setPositionField("left", e.target.value)}
              />
            </div>
            <div style={halfStyle}>
              <TextField
                label="Right"
                value={config.button_position.right}
                onChange={(e) => setPositionField("right", e.target.value)}
              />
            </div>
          </Focusable>
        </PanelSectionRow>
      </PanelSection>

      <PanelSection>
        <PanelSectionRow>
          <ButtonItem layout="below" onClick={onSave}>
            Salvar
          </ButtonItem>
        </PanelSectionRow>
      </PanelSection>
    </>
  );
}
