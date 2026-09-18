import { Dialog } from "@base-ui/react/dialog";
import { useRef, useState, type RefObject } from "react";
import type { MessageKey } from "../../i18n";
import { Button } from "./button";

export function NameDialog({ name, t, onSave, onClose, returnFocus }: {
  name: string;
  t: (key: MessageKey) => string;
  onSave: (name: string) => Promise<void>;
  onClose: () => void;
  returnFocus: RefObject<HTMLButtonElement | null>;
}) {
  const [draft, setDraft] = useState(name);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const input = useRef<HTMLInputElement>(null);
  const saving = useRef(false);
  async function save() {
    if (saving.current || !draft.trim() || draft.trim() === name) return;
    saving.current = true;
    setBusy(true);
    setError(undefined);
    try { await onSave(draft.trim()); onClose(); }
    catch (reason) { setError(String(reason)); }
    finally { saving.current = false; setBusy(false); }
  }
  return <Dialog.Root open onOpenChange={(open) => { if (!open && !saving.current) onClose(); }}>
    <Dialog.Portal>
      <Dialog.Backdrop className="dialog-backdrop" />
      <Dialog.Popup className="dialog-popup name-dialog" finalFocus={returnFocus} initialFocus={() => { input.current?.select(); return input.current; }}>
        <form onKeyDown={(event) => { if (event.key === "Enter" && event.nativeEvent.isComposing) event.preventDefault(); }} onSubmit={(event) => { event.preventDefault(); void save(); }}>
          <Dialog.Title className="dialog-title">{t("editName")}</Dialog.Title>
          <Dialog.Description className="dialog-description">{t("projectNameHint")}</Dialog.Description>
          <label className="field-group"><span>{t("projectName")}</span><input ref={input} value={draft} disabled={busy} required onChange={(event) => setDraft(event.target.value)} /></label>
          {error && <p className="form-error" role="alert">{t("saveFailed")}: {error}</p>}
          <div className="dialog-actions"><Dialog.Close render={<Button type="button" disabled={busy}>{t("cancel")}</Button>} /><Button type="submit" variant="primary" loading={busy} disabled={!draft.trim() || draft.trim() === name}>{t("save")}</Button></div>
        </form>
      </Dialog.Popup>
    </Dialog.Portal>
  </Dialog.Root>;
}
