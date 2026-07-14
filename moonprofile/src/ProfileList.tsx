import { CSSProperties } from "react";
import { PanelSection, PanelSectionRow, ButtonItem, DialogButton, Focusable, Field, showModal, ConfirmModal } from "@decky/ui";
import { Profile } from "./types";

// "ButtonItem" e' pensado pra ocupar a row inteira sozinho (extends
// "ItemProps", como um Field completo) - colocar varios dentro do mesmo
// Field empilha em vez de ficar lado a lado. "DialogButton" e' o botao
// "de verdade" (com fundo cinza, borda arredondada, padding - o mesmo visual
// usado por baixo dos panos em ButtonItem/ConfirmModal); o "Button" puro
// (usado no GameActionButton) e' so' o elemento interno, sem esse invólucro
// visual - por isso ficava "cru" aqui.
const buttonRowStyle: CSSProperties = { display: "flex", flexDirection: "row", gap: "8px" };
const buttonStyle: CSSProperties = { flexGrow: 1, minWidth: 0 };

interface ProfileListProps {
  profiles: Profile[];
  onNew: () => void;
  onEdit: (profile: Profile) => void;
  onDuplicate: (profile: Profile) => void;
  onDelete: (profile: Profile) => void;
}

export function ProfileList({ profiles, onNew, onEdit, onDuplicate, onDelete }: ProfileListProps) {
  const confirmDelete = (profile: Profile) => {
    showModal(
      <ConfirmModal
        strTitle="Excluir perfil"
        strDescription={`Tem certeza que quer excluir "${profile.name}"?`}
        onOK={() => onDelete(profile)}
      />,
    );
  };

  return (
    <PanelSection>
      {profiles.length === 0 && <PanelSectionRow>Nenhum perfil configurado</PanelSectionRow>}
      {profiles.map((p) => (
        <PanelSectionRow key={p.id}>
          <Field label={p.name} description={p.trigger}>
            <Focusable style={buttonRowStyle}>
              <DialogButton style={buttonStyle} onClick={() => onEdit(p)}>
                Editar
              </DialogButton>
              <DialogButton style={buttonStyle} onClick={() => onDuplicate(p)}>
                Duplicar
              </DialogButton>
              <DialogButton style={buttonStyle} onClick={() => confirmDelete(p)}>
                Excluir
              </DialogButton>
            </Focusable>
          </Field>
        </PanelSectionRow>
      ))}
      <PanelSectionRow>
        <ButtonItem layout="below" onClick={onNew}>
          Novo perfil
        </ButtonItem>
      </PanelSectionRow>
    </PanelSection>
  );
}
