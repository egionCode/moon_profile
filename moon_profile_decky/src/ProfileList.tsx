import { CSSProperties } from "react";
import { PanelSection, PanelSectionRow, ButtonItem, DialogButton, Focusable, Field, showModal, ConfirmModal } from "@decky/ui";
import { Profile } from "./types";

// "ButtonItem" is meant to occupy the whole row by itself (extends
// "ItemProps", like a full Field), placing several inside the same Field
// stacks them instead of putting them side by side. "DialogButton" is the
// "real" button (gray background, rounded border, padding, the same look
// used under the hood in ButtonItem/ConfirmModal); the plain "Button"
// (used in GameActionButton) is just the inner element, without that
// visual wrapper, which is why it looked "raw" here.
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
        strTitle="Delete profile"
        strDescription={`Are you sure you want to delete "${profile.name}"?`}
        onOK={() => onDelete(profile)}
      />,
    );
  };

  return (
    <PanelSection>
      {profiles.length === 0 && <PanelSectionRow>No profile configured</PanelSectionRow>}
      {profiles.map((p) => (
        <PanelSectionRow key={p.id}>
          <Field label={p.name} description={p.trigger}>
            <Focusable style={buttonRowStyle}>
              <DialogButton style={buttonStyle} onClick={() => onEdit(p)}>
                Edit
              </DialogButton>
              <DialogButton style={buttonStyle} onClick={() => onDuplicate(p)}>
                Duplicate
              </DialogButton>
              <DialogButton style={buttonStyle} onClick={() => confirmDelete(p)}>
                Delete
              </DialogButton>
            </Focusable>
          </Field>
        </PanelSectionRow>
      ))}
      <PanelSectionRow>
        <ButtonItem layout="below" onClick={onNew}>
          New profile
        </ButtonItem>
      </PanelSectionRow>
    </PanelSection>
  );
}
