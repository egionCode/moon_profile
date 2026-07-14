import { PanelSection, PanelSectionRow, TextField, ButtonItem, DialogBodyText } from "@decky/ui";
import { Config } from "./types";

interface ApolloConfigSectionProps {
  config: Config;
  setConfig: (config: Config) => void;
  onSave: () => void;
}

// Aba "Config do Apollo" da sidenav de Configuracoes (SettingsPage.tsx) -
// so' host/usuario/senha, com Salvar proprio. O estado ("config") e' dono
// do SettingsPage, nao daqui - assim trocar de aba sem salvar nao perde
// edicoes feitas na aba de posicionamento do botao (as duas mexem no MESMO
// objeto Config, que e' salvo inteiro de uma vez so no backend).
export function ApolloConfigSection({ config, setConfig, onSave }: ApolloConfigSectionProps) {
  return (
    <>
      <PanelSection>
        <PanelSectionRow>
          <DialogBodyText>
            Essas sao as mesmas credenciais de admin que voce usa pra entrar no painel web do
            Apollo (o servidor de streaming rodando no seu PC/host). O plugin usa a API do Apollo
            com elas pra configurar a tela do host e iniciar as sessoes de streaming automaticamente
            - nada e enviado pra fora da sua rede local.
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
            label="Usuario"
            value={config.username}
            onChange={(e) => setConfig({ ...config, username: e.target.value })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Senha"
            bIsPassword
            value={config.password}
            onChange={(e) => setConfig({ ...config, password: e.target.value })}
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
