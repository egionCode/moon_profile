import { afterPatch, wrapReactType } from "@decky/ui";
import { routerHook } from "@decky/api";
import { GameActionButton } from "../GameActionButton";

// "any" de proposito nesse arquivo inteiro: e' introspeccao estrutural na
// arvore interna de React da Steam, sem tipo nenhum documentado - nao da
// pra tipar de verdade (mesma abordagem que o hltb-for-deck usa).
/* eslint-disable @typescript-eslint/no-explicit-any */

// Adaptado do padrao usado pelo hltb-for-deck (github.com/hulkrelax/
// hltb-for-deck, src/patches/LibraryApp.tsx - referencia que o proprio PRD
// indica pra esse tipo de patch). Eles mesmos comentam "I hate this
// method" - e' assim mesmo: acha o container de acoes da tela do jogo
// (Jogar/Stream From) navegando a arvore de React por heuristica de props
// (childFocusDisabled/navRef/details/overview/bFastRender), nao por algo
// documentado. QUEBRA FACIL entre versoes do Steam client - se o botao
// sumir depois de uma atualizacao da Steam, e' aqui que mexer primeiro.
const MARKER_ID = "moonprofile-stream-button";

export function patchLibraryApp() {
  return routerHook.addPatch(
    "/library/app/:appid",
    (props: { path: string; children: any }) => {
      afterPatch(
        props.children.props,
        "renderFunc",
        (_: Record<string, unknown>[], ret1: any) => {
          const appId: number = ret1.props.children.props.overview.appid;
          wrapReactType(ret1.props.children);
          afterPatch(
            ret1.props.children.type,
            "type",
            (_1: Record<string, unknown>[], ret2: any) => {
              const componentToSplice =
                ret2.props.children?.[1]?.props.children.props.children;

              const existingIndex = componentToSplice?.findIndex(
                (child: any) => child?.props?.id === MARKER_ID,
              );

              // O item de referencia pra saber ONDE encaixar (logo antes
              // dele) - identificado pela combinacao de props que so' o
              // container de acoes principal (Jogar/etc) tem.
              const spliceIndex = componentToSplice?.findIndex(
                (child: any) => {
                  return (
                    child?.props?.childFocusDisabled !== undefined &&
                    child?.props?.navRef !== undefined &&
                    child?.props?.children?.props?.details !== undefined &&
                    child?.props?.children?.props?.overview !== undefined &&
                    child?.props?.children?.props?.bFastRender !== undefined
                  );
                },
              );

              const component = (
                <GameActionButton key={MARKER_ID} id={MARKER_ID} appId={appId} />
              );

              if (existingIndex === undefined || existingIndex < 0) {
                if (spliceIndex !== undefined && spliceIndex > -1) {
                  componentToSplice.splice(spliceIndex, 0, component);
                } else {
                  console.error(
                    "MoonProfile: nao achei onde injetar o botao na tela do jogo (estrutura da UI da Steam mudou?)",
                  );
                }
              } else {
                componentToSplice.splice(existingIndex, 1, component);
              }

              return ret2;
            },
          );
          return ret1;
        },
      );
      return props;
    },
  );
}
