import { DialogButton, Focusable, Navigation, staticClasses } from "@decky/ui";
import { FaCog } from "react-icons/fa";
import { SETTINGS_ROUTE } from "./routes";

// Pattern copied from MoonDeck (src/components/titleview/titleview.tsx)
// and checked against CssLoader (DeckThemes/SDH-CssLoader,
// src/components/TitleView.tsx), both agree on the exact same values, so
// these are the "correct" ones here, not a coincidence: "padding: 0" and
// "boxShadow: none" on the Focusable cancel out the padding/shadow that the
// "staticClasses.Title" class already brings (without this, they stack and
// misalign); "marginRight: auto" on the title explicitly pushes it to the
// left; "marginTop: -4px" on the icon re-centers it vertically inside the
// 28px button (without this the icon is visibly shifted downward).
export function TitleView() {
  const onSettingsClick = () => {
    Navigation.CloseSideMenus();
    Navigation.Navigate(SETTINGS_ROUTE);
  };

  return (
    <Focusable
      style={{
        display: "flex",
        padding: "0",
        width: "100%",
        boxShadow: "none",
        alignItems: "center",
        justifyContent: "space-between",
      }}
      className={staticClasses.Title}
    >
      <div style={{ marginRight: "auto" }}>MoonProfile</div>
      <DialogButton
        style={{ height: "28px", width: "40px", minWidth: 0, padding: "10px 12px" }}
        onClick={onSettingsClick}
      >
        <FaCog style={{ marginTop: "-4px", display: "block" }} />
      </DialogButton>
    </Focusable>
  );
}
