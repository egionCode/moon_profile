import { DialogButton, Focusable, Navigation, staticClasses } from "@decky/ui";
import { FaCog } from "react-icons/fa";
import { SETTINGS_ROUTE } from "./routes";

// Padrao copiado do MoonDeck (src/components/titleview/titleview.tsx) e
// conferido contra o CssLoader (DeckThemes/SDH-CssLoader,
// src/components/TitleView.tsx) - os dois concordam exatamente nos mesmos
// valores, entao sao o "certo" aqui, nao coincidencia: "padding: 0" e
// "boxShadow: none" no Focusable anulam o padding/sombra que a propria
// classe "staticClasses.Title" ja' traz (sem isso, soma e desalinha);
// "marginRight: auto" no titulo empurra ele pra esquerda explicitamente;
// "marginTop: -4px" no icone recentraliza verticalmente dentro do botao
// de 28px (sem isso o icone fica visivelmente deslocado pra baixo).
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
