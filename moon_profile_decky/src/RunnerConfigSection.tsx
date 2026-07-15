import { PanelSection, PanelSectionRow, TextField, ButtonItem, DialogBodyText } from "@decky/ui";
import { Config } from "./types";

interface RunnerConfigSectionProps {
  config: Config;
  setConfig: (config: Config) => void;
  onSave: () => void;
}

// Aba "Runner" da sidenav de Configuracoes - MoonProfile Runner e' o daemon
// Tauri/Rust que roda no host (Fase 5 do PRD). NAO e' mais opcional: o
// Apollo nao tem prep-cmd nenhum (nem do, nem undo) - e' o Runner quem
// liga a tela no lancamento e desliga no fechamento, alem de detectar
// sozinho quando o jogo fecha por dentro. So' a porta e' configuravel
// aqui - o host e' sempre o mesmo da aba "Config do Apollo" (Runner e
// Apollo rodam na mesma maquina, pedir o IP duas vezes seria redundante
// e confuso).
export function RunnerConfigSection({ config, setConfig, onSave }: RunnerConfigSectionProps) {
  return (
    <>
      <PanelSection>
        <PanelSectionRow>
          <DialogBodyText>
            Obrigatorio - o MoonProfile Runner precisa estar instalado e rodando no host pra
            trocar a tela (resolucao/monitor) no lancamento e na desconexao, e pra detectar
            automaticamente quando o jogo fecha por dentro. Usa o mesmo host configurado na aba
            "Config do Apollo" - servidor local aberto na rede, sem autenticacao.
          </DialogBodyText>
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
