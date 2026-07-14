import { useState, useEffect, FC, ReactNode, CSSProperties } from "react";
import { toaster } from "@decky/api";
import {
  ScrollPanelGroup as ScrollPanelGroupUntyped,
  ScrollPanel as ScrollPanelUntyped,
  Focusable,
  Navigation,
  QuickAccessTab,
  SidebarNavigation,
} from "@decky/ui";
import { ProfileList } from "./ProfileList";
import { ProfileEditor } from "./ProfileEditor";
import { ApolloConfigSection } from "./ApolloConfigSection";
import { ButtonPositionSection } from "./ButtonPositionSection";
import { RunnerConfigSection } from "./RunnerConfigSection";
import { LogsSection } from "./LogsSection";
import { getProfiles, saveProfiles, getConfig, saveConfig } from "./api";
import { Config, Profile } from "./types";

// @decky/ui so' tipa "children" pra esses dois (sao modulos mapeados via
// webpack em tempo de execucao, sem .d.ts fiel) mas aceitam style/focusable
// de verdade - mesmo par que a propria Steam usa pra scroll com navegacao
// por gamepad.
type ScrollProps = { children?: ReactNode; style?: CSSProperties; focusable?: boolean };
const ScrollPanelGroup = ScrollPanelGroupUntyped as FC<ScrollProps>;
const ScrollPanel = ScrollPanelUntyped as FC<ScrollProps>;

function blankProfile(): Profile {
  return {
    id: "",
    name: "",
    trigger: "manual",
    moonlight: { resolution: "1920x1080", fps: 60, bitrate: 20000, codec: "HEVC", hdr: false },
    host: { target_output: "", resolution: "1920x1080", fps: 60, hdr: false, wcg: false, disable_outputs: [] },
  };
}

// Gera um id novo unico a partir do nome (slug) - usado tanto pro "Novo
// perfil" quanto pro "Duplicar" (que precisa de um id diferente do original).
function makeUniqueId(base: string, existingIds: string[]): string {
  const slug = base.toLowerCase().trim().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "perfil";
  if (!existingIds.includes(slug)) {
    return slug;
  }
  let i = 2;
  while (existingIds.includes(`${slug}-${i}`)) {
    i++;
  }
  return `${slug}-${i}`;
}

// Pagina cheia de configuracoes, aberta via routerHook (ver index.tsx) a
// partir do icone de engrenagem no Quick Access. Sidenav nativa da Steam
// (SidebarNavigation) com 3 abas - Config do Apollo (default), Posicao do
// botao e Perfis - cada uma das duas primeiras com Salvar proprio; "config"
// e' dono daqui e passado pras duas, entao trocar de aba sem salvar nao
// perde edicao feita na outra (as duas mexem no MESMO objeto, salvo
// inteiro de uma vez so no backend).
export function SettingsPage() {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [config, setConfig] = useState<Config | null>(null);
  const [editing, setEditing] = useState<{ profile: Profile; isNew: boolean } | null>(null);

  useEffect(() => {
    getProfiles().then(setProfiles);
    getConfig().then(setConfig);
  }, []);

  const persist = async (updated: Profile[], message: string) => {
    setProfiles(updated);
    await saveProfiles(updated);
    toaster.toast({ title: "MoonProfile", body: message });
  };

  const onSaveConfig = async () => {
    if (!config) {
      return;
    }
    await saveConfig(config);
    toaster.toast({ title: "MoonProfile", body: "Config salva" });
  };

  const closeEditor = () => {
    setEditing(null);
  };

  const onNew = () => {
    setEditing({ profile: blankProfile(), isNew: true });
  };

  const onEdit = (profile: Profile) => {
    setEditing({ profile, isNew: false });
  };

  const onDuplicate = (profile: Profile) => {
    const newId = makeUniqueId(`${profile.id}-copy`, profiles.map((p) => p.id));
    setEditing({ profile: { ...profile, id: newId, name: `${profile.name} (copia)` }, isNew: true });
  };

  const onDelete = (profile: Profile) => {
    void persist(profiles.filter((p) => p.id !== profile.id), `Perfil "${profile.name}" excluido`);
  };

  const onSaveFromEditor = (saved: Profile) => {
    const exists = profiles.some((p) => p.id === saved.id);
    const updated = exists ? profiles.map((p) => (p.id === saved.id ? saved : p)) : [...profiles, saved];
    void persist(updated, `Perfil "${saved.name}" salvo`);
    closeEditor();
  };

  // routerHook.addRoute monta o componente puro, via createElement(component)
  // sem nenhuma prop (confirmado na fonte do decky-loader,
  // frontend/src/router-hook.tsx) - entao nao ha' "history"/"location" pra
  // usar aqui. O botao fisico B do Steam Deck nao navega o history do
  // browser nessa UI - dispara um CustomEvent "cancel" que sobe pela arvore
  // de Focusable ate' alguem tratar (documentado no proprio FocusableProps
  // do @decky/ui: onCancel). Por isso a interceptacao certa e' via
  // Focusable.onCancel, nao via history/routing.
  //
  // Editando um perfil: consome o evento e volta pra listagem (nao sai de
  // Config.). Fora disso (em qualquer aba da sidenav): sai da rota de
  // Config. e reabre o Quick Access na aba do Decky - simetrico com o botao
  // de engrenagem (TitleView.tsx), que faz CloseSideMenus() +
  // Navigate(SETTINGS_ROUTE) pra chegar aqui.
  const onCancelButton = (e: CustomEvent) => {
    e.stopPropagation();
    if (editing !== null) {
      closeEditor();
      return;
    }
    Navigation.NavigateBack();
    Navigation.OpenQuickAccessMenu(QuickAccessTab.Decky);
  };

  if (!config) {
    return null;
  }

  const pages = [
    {
      title: "Config do Apollo",
      identifier: "apollo",
      content: <ApolloConfigSection config={config} setConfig={setConfig} onSave={onSaveConfig} />,
    },
    {
      title: "Posicao do botao",
      identifier: "button-position",
      content: <ButtonPositionSection config={config} setConfig={setConfig} onSave={onSaveConfig} />,
    },
    {
      title: "Perfis",
      identifier: "profiles",
      content: editing ? (
        <ProfileEditor
          profile={editing.profile}
          isNew={editing.isNew}
          existingIds={profiles.map((p) => p.id)}
          onSave={onSaveFromEditor}
          onCancel={closeEditor}
        />
      ) : (
        <ProfileList
          profiles={profiles}
          onNew={onNew}
          onEdit={onEdit}
          onDuplicate={onDuplicate}
          onDelete={onDelete}
        />
      ),
    },
    {
      title: "Runner",
      identifier: "runner",
      content: <RunnerConfigSection config={config} setConfig={setConfig} onSave={onSaveConfig} />,
    },
    {
      title: "Logs",
      identifier: "logs",
      content: <LogsSection />,
    },
  ];

  return (
    // Rota custom via routerHook.addRoute - diferente das telas nativas da
    // Steam, nao vem com scroll de graca. ScrollPanelGroup/ScrollPanel e' o
    // mesmo par que a propria Steam usa pra isso (respeita navegacao por
    // gamepad tambem, ao contrario de so' um overflow:auto cru).
    <ScrollPanelGroup style={{ height: "100%" }} focusable={false}>
      <ScrollPanel style={{ height: "100%" }}>
        <Focusable onCancel={onCancelButton} style={{ height: "100%" }}>
          {/* paddingBottom generoso - sem isso o fim do conteudo fica atras
              da barra inferior do Steam Deck, cortando a visualizacao. */}
          <div style={{ height: "100%", paddingBottom: "60px" }}>
            <SidebarNavigation title="MoonProfile" pages={pages} />
          </div>
        </Focusable>
      </ScrollPanel>
    </ScrollPanelGroup>
  );
}
