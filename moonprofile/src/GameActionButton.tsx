import { CSSProperties, useState, useEffect } from "react";
import { Button, Focusable, joinClassNames, playSectionClasses, basicAppDetailsSectionStylerClasses } from "@decky/ui";
import { toaster } from "@decky/api";
import { FaSatelliteDish } from "react-icons/fa";
import { runStream } from "./stream";
import { getConfig } from "./api";
import { ButtonPosition } from "./types";

interface GameActionButtonProps {
  appId: number;
  // Nao usado pro render - so' serve de marca pro LibraryAppPatch achar essa
  // instancia de novo (evitar duplicar ao re-patchar a mesma pagina).
  id?: string;
}

// Mesmo default que o backend usa (main.py: DEFAULT_BUTTON_POSITION) -
// so' o valor inicial ate' o getConfig() de verdade responder.
const DEFAULT_POSITION: ButtonPosition = { top: "", bottom: "2.8vw", left: "32px", right: "" };

// Visual copiado do MoonDeck (src/components/moondecklaunchbutton/
// moondecklaunchbutton.tsx): "Button" + as classes CSS REAIS que o proprio
// botao "Jogar" da Steam usa (playSectionClasses.MenuButton dentro de
// basicAppDetailsSectionStylerClasses.AppButtons) - fica visualmente
// consistente com os botoes nativos em vez de uma caixa colorida generica.
export function GameActionButton({ appId }: GameActionButtonProps) {
  const [streaming, setStreaming] = useState(false);
  const [position, setPosition] = useState<ButtonPosition>(DEFAULT_POSITION);

  // O Steam mantem a pagina do jogo viva num "backstack" ao navegar pra
  // Configuracoes e voltar (nao desmonta de verdade) - um useEffect(() => {},
  // []) sozinho so buscaria o config UMA vez por sessao, presa no valor de
  // quando a pagina abriu pela primeira vez (confirmado: o config.json ja
  // tinha o preset novo salvo, mas o botao nao se mexia ao voltar pro mesmo
  // jogo). Poll leve enquanto montado resolve sem precisar de um event bus
  // global so' pra isso.
  useEffect(() => {
    const load = () => getConfig().then((c) => setPosition(c.button_position ?? DEFAULT_POSITION));
    load();
    const interval = setInterval(load, 3000);
    return () => clearInterval(interval);
  }, []);

  // O ponto onde este componente e' injetado (dentro do bloco de acoes do
  // jogo) fica ABAIXO do hero/banner - com "position: absolute", "top" conta
  // a partir desse ancestral local (ja abaixo do hero), entao nunca alcanca
  // de verdade a area do hero (confirmado: "top" ficava logo abaixo dele).
  // "position: fixed" ancora na viewport de verdade, ignorando onde o
  // componente foi spliced na arvore - por isso so' usamos "top" com fixed;
  // "bottom" continua com absolute (o ancestral local ja fica perto de onde
  // o botao "Jogar" nativo esta, entao absolute funciona bem ali).
  const usingTopAnchor = Boolean(position.top);
  const containerStyle: CSSProperties = {
    position: usingTopAnchor ? "fixed" : "absolute",
    zIndex: 10,
    ...(usingTopAnchor ? { top: position.top } : {}),
    // "|| DEFAULT_POSITION.bottom": se "bottom" estiver vazio (campos
    // limpos na mao, sem "top" tambem), cai num padding minimo em vez de
    // ir pro "0" da borda e ficar em cima do botao "Jogar" nativo.
    ...(!usingTopAnchor ? { bottom: position.bottom || DEFAULT_POSITION.bottom } : {}),
    ...(position.left ? { left: position.left } : {}),
    ...(position.right ? { right: position.right } : {}),
  };

  const onClick = async () => {
    if (streaming) {
      return;
    }
    setStreaming(true);
    try {
      await runStream(appId);
    } catch (e) {
      console.error("MoonProfile: erro inesperado no stream (botao da tela do jogo)", e);
      toaster.toast({ title: "MoonProfile - erro inesperado", body: String(e) });
    } finally {
      setStreaming(false);
    }
  };

  return (
    // O <div style={{position: "relative", height: 0}}> e' a "ancora": sem
    // ele, o "position: absolute" do container do botao acaba se referenciando
    // a um ancestral generico bem mais distante na arvore (a pagina toda,
    // por exemplo), fazendo o botao aparecer fora do lugar (confirmado
    // testando no device). Com o ancestral mais proximo virando "relative"
    // exatamente onde o React splica esse componente, o absolute passa a
    // se referenciar a ESSE ponto. Mesma tecnica exata do MoonDeck
    // (moondecklaunchbutton.tsx: <div id="moondeck" style={{position:...}}>).
    <div style={{ position: "relative", height: 0 }}>
      <Focusable
        style={containerStyle}
        className={joinClassNames(basicAppDetailsSectionStylerClasses.AppButtons, "moonprofile-container")}
      >
        <Button
          disabled={streaming}
          className={joinClassNames(playSectionClasses.MenuButton, "moonprofile-button")}
          onClick={onClick}
        >
          <FaSatelliteDish />
        </Button>
      </Focusable>
    </div>
  );
}
