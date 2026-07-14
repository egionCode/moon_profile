import { PanelSection, PanelSectionRow, TextField, ButtonItem, DialogBodyText } from "@decky/ui";
import { Config } from "./types";

interface RunnerConfigSectionProps {
  config: Config;
  setConfig: (config: Config) => void;
  onSave: () => void;
}

// Aba "Runner" da sidenav de Configuracoes - MoonProfile Runner e' o daemon
// Tauri/Rust que roda no host (Fase 5 do PRD), suprindo a deteccao de fim
// de sessao que o Apollo nao consegue fazer sozinho. Mesmo padrao de
// dono-do-estado-e-o-pai que ApolloConfigSection.tsx/ButtonPositionSection.tsx.
export function RunnerConfigSection({ config, setConfig, onSave }: RunnerConfigSectionProps) {
  return (
    <>
      <PanelSection>
        <PanelSectionRow>
          <DialogBodyText>
            Opcional - so' necessario se voce instalou o MoonProfile Runner no host (detecta
            automaticamente quando o jogo fecha por dentro, sem precisar clicar "Fechar conexao").
            Servidor local aberto na rede, sem autenticacao - so' preencher host e porta.
          </DialogBodyText>
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Host"
            value={config.runner_host}
            onChange={(e) => setConfig({ ...config, runner_host: e.target.value })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Porta"
            mustBeNumeric
            value={String(config.runner_port)}
            onChange={(e) => setConfig({ ...config, runner_port: Number(e.target.value) || 0 })}
          />
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
