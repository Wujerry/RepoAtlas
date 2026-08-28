import { Dialog } from "@base-ui/react/dialog";
import { Button } from "./button";

export function DescriptionDialog({ open, value, saving, title, label, placeholder, saveLabel, cancelLabel, onValue, onOpenChange, onSave }: {
  open: boolean;
  value: string;
  saving: boolean;
  title: string;
  label: string;
  placeholder: string;
  saveLabel: string;
  cancelLabel: string;
  onValue: (value: string) => void;
  onOpenChange: (open: boolean) => void;
  onSave: () => void | Promise<void>;
}) {
  return <Dialog.Root open={open} onOpenChange={(next) => !saving && onOpenChange(next)}>
    <Dialog.Portal>
      <Dialog.Backdrop className="dialog-backdrop" />
      <Dialog.Popup className="dialog-popup description-dialog">
        <Dialog.Title className="dialog-title">{title}</Dialog.Title>
        <label className="field-group"><span>{label}</span><textarea autoFocus value={value} placeholder={placeholder} maxLength={600} onChange={(event) => onValue(event.target.value)} /></label>
        <div className="description-counter">{value.length} / 600</div>
        <div className="dialog-actions"><Dialog.Close render={<Button disabled={saving}>{cancelLabel}</Button>} /><Button variant="primary" loading={saving} onClick={() => void onSave()}>{saveLabel}</Button></div>
      </Dialog.Popup>
    </Dialog.Portal>
  </Dialog.Root>;
}
