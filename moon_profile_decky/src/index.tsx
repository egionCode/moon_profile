import { definePlugin, routerHook } from "@decky/api";
import { FaSatelliteDish } from "react-icons/fa";
import { QuickAccessContent } from "./QuickAccessContent";
import { SettingsPage } from "./SettingsPage";
import { TitleView } from "./TitleView";
import { SETTINGS_ROUTE } from "./routes";

export default definePlugin(() => {
  routerHook.addRoute(SETTINGS_ROUTE, SettingsPage, { exact: true });

  return {
    name: "MoonProfile",
    titleView: <TitleView />,
    content: <QuickAccessContent />,
    icon: <FaSatelliteDish />,
    onDismount() {
      routerHook.removeRoute(SETTINGS_ROUTE);
    },
  };
});
