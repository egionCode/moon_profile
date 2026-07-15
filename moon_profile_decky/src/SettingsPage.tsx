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
import { RunnerConfigSection } from "./RunnerConfigSection";
import { GamesGridSection } from "./GamesGridSection";
import { LogsSection } from "./LogsSection";
import { getProfiles, saveProfiles, getConfig, saveConfig } from "./api";
import { Config, Profile } from "./types";

// @decky/ui only types "children" for these two (they're modules mapped
// via webpack at runtime, without a faithful .d.ts) but they do accept a
// real style/focusable, the same pair Steam itself uses for scrolling with
// gamepad navigation.
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

// Generates a new unique id from the name (slug), used both for "New
// profile" and for "Duplicate" (which needs an id different from the
// original).
function makeUniqueId(base: string, existingIds: string[]): string {
  const slug = base.toLowerCase().trim().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "profile";
  if (!existingIds.includes(slug)) {
    return slug;
  }
  let i = 2;
  while (existingIds.includes(`${slug}-${i}`)) {
    i++;
  }
  return `${slug}-${i}`;
}

// Full settings page, opened via routerHook (see index.tsx) from the gear
// icon in Quick Access. Steam's native sidenav (SidebarNavigation): Apollo
// Config (default), Profiles, Runner, Games, and Logs. "config" is owned
// here and passed to the tabs that touch it (Apollo/Runner), so switching
// tabs without saving doesn't lose edits made in the other one (they touch
// the SAME object, saved whole in one go on the backend).
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
    toaster.toast({ title: "MoonProfile", body: "Config saved" });
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
    setEditing({ profile: { ...profile, id: newId, name: `${profile.name} (copy)` }, isNew: true });
  };

  const onDelete = (profile: Profile) => {
    void persist(profiles.filter((p) => p.id !== profile.id), `Profile "${profile.name}" deleted`);
  };

  const onSaveFromEditor = (saved: Profile) => {
    const exists = profiles.some((p) => p.id === saved.id);
    const updated = exists ? profiles.map((p) => (p.id === saved.id ? saved : p)) : [...profiles, saved];
    void persist(updated, `Profile "${saved.name}" saved`);
    closeEditor();
  };

  // routerHook.addRoute mounts the component plainly, via
  // createElement(component) with no props at all (confirmed in
  // decky-loader's source, frontend/src/router-hook.tsx), so there's no
  // "history"/"location" to use here. The Steam Deck's physical B button
  // doesn't navigate the browser's history in this UI, it fires a
  // CustomEvent "cancel" that bubbles up the Focusable tree until someone
  // handles it (documented in @decky/ui's own FocusableProps: onCancel).
  // That's why the correct interception point is Focusable.onCancel, not
  // history/routing.
  //
  // Editing a profile: consumes the event and goes back to the listing
  // (doesn't leave the Settings page). Otherwise (in any sidenav tab):
  // leaves the Settings route and reopens Quick Access on the Decky tab,
  // symmetric with the gear button (TitleView.tsx), which does
  // CloseSideMenus() + Navigate(SETTINGS_ROUTE) to get here.
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
      title: "Apollo Config",
      identifier: "apollo",
      content: <ApolloConfigSection config={config} setConfig={setConfig} onSave={onSaveConfig} />,
    },
    {
      title: "Profiles",
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
      title: "Games",
      identifier: "games",
      content: <GamesGridSection />,
    },
    {
      title: "Logs",
      identifier: "logs",
      content: <LogsSection />,
    },
  ];

  return (
    // Custom route via routerHook.addRoute, unlike Steam's native screens,
    // it doesn't come with scrolling for free. ScrollPanelGroup/ScrollPanel
    // is the same pair Steam itself uses for this (also respects gamepad
    // navigation, unlike a plain overflow:auto).
    <ScrollPanelGroup style={{ height: "100%" }} focusable={false}>
      <ScrollPanel style={{ height: "100%" }}>
        <Focusable onCancel={onCancelButton} style={{ height: "100%" }}>
          {/* Generous paddingBottom, without it the end of the content ends
              up behind the Steam Deck's bottom bar, cutting off the view. */}
          <div style={{ height: "100%", paddingBottom: "60px" }}>
            <SidebarNavigation title="MoonProfile" pages={pages} />
          </div>
        </Focusable>
      </ScrollPanel>
    </ScrollPanelGroup>
  );
}
