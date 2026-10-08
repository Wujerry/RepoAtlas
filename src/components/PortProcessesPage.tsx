import { Dialog } from "@base-ui/react/dialog";
import type { MessageKey } from "../i18n";
import type { PortConflictContext } from "../lib/port-processes";
import { PortProcesses } from "./PortProcesses";

export function PortProcessesPage({ open, t, context, onClose, onJump }: {
  open: boolean; t: (key: MessageKey) => string; context?: PortConflictContext;
  onClose: () => void; onJump: (id: string) => void;
}) {
  return <Dialog.Root modal={false} open={open} onOpenChange={(next, details) => {
    if (!next && details.reason !== "outside-press" && details.reason !== "focus-out") onClose();
  }}><Dialog.Portal><Dialog.Popup className="port-process-page" finalFocus={() => document.querySelector<HTMLButtonElement>(".titlebar-port-processes")}>
    <Dialog.Title className="sr-only">{t("portProcesses")}</Dialog.Title>
    <Dialog.Description className="sr-only">{t("processScopeHint")}</Dialog.Description>
    {open && <PortProcesses t={t} context={context} onJump={onJump} onClose={onClose} />}
  </Dialog.Popup></Dialog.Portal></Dialog.Root>;
}
