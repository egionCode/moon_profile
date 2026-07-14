import { definePlugin, routerHook } from "@decky/api";
import { FaSatelliteDish } from "react-icons/fa";
import { QuickAccessContent } from "./QuickAccessContent";
import { SettingsPage } from "./SettingsPage";
import { TitleView } from "./TitleView";
import { SETTINGS_ROUTE } from "./routes";
import { patchLibraryApp } from "./patches/LibraryAppPatch";

const LIBRARY_APP_ROUTE = "/library/app/:appid";

export default definePlugin(() => {
  routerHook.addRoute(SETTINGS_ROUTE, SettingsPage, { exact: true });
  const libraryAppPatch = patchLibraryApp();

  return {
    name: "MoonProfile",
    titleView: <TitleView />,
    content: <QuickAccessContent />,
    icon: <FaSatelliteDish />,
    onDismount() {
      routerHook.removeRoute(SETTINGS_ROUTE);
      routerHook.removePatch(LIBRARY_APP_ROUTE, libraryAppPatch);
    },
  };
});
