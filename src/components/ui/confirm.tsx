import { AlertDialog } from "@base-ui/react/alert-dialog";
import { Button } from "./button";

export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  cancelLabel = "Cancel",
  busy = false,
  onOpenChange,
  onConfirm,
  className = "",
  confirmVariant = "danger",
}: {
  open: boolean;
  title: string;
  body: string;
  confirmLabel: string;
  cancelLabel?: string;
  busy?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void | Promise<void>;
  className?: string;
  confirmVariant?: "primary" | "danger";
}) {
  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Portal>
        <AlertDialog.Backdrop className={`dialog-backdrop ${className}`} />
        <AlertDialog.Popup className={`dialog-popup ${className}`}>
          <AlertDialog.Title className="dialog-title">{title}</AlertDialog.Title>
          <AlertDialog.Description className="dialog-description">{body}</AlertDialog.Description>
          <div className="dialog-actions">
            <AlertDialog.Close render={<Button disabled={busy}>{cancelLabel}</Button>} />
            <Button variant={confirmVariant} loading={busy} disabled={busy} onClick={() => void onConfirm()}>{confirmLabel}</Button>
          </div>
        </AlertDialog.Popup>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
