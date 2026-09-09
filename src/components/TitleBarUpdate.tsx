import { ArrowClockwise, CircleNotch, DownloadSimple, Package, WarningCircle } from "@phosphor-icons/react";
import { Popover } from "@base-ui/react/popover";
import { useEffect, useState } from "react";
import type { MessageKey } from "../i18n";
import { formatUpdateBytes, formatUpdateDate, updateErrorTitle } from "../lib/update-format";
import type { UpdateState } from "../types";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";

type Action = "check" | "download" | "install" | "restart";
type ConfirmAction = "install" | "restart";

export interface TitleBarUpdateProps {
  state: UpdateState;
  locale?: string;
  t: (key: MessageKey) => string;
  onCheck: () => Promise<UpdateState>;
  onDownload: () => Promise<UpdateState>;
  onInstall: () => Promise<boolean>;
  onRestart: () => Promise<void>;
  onDefer: () => Promise<void>;
}

/**
 * The entry appears only once an update is actually in play. A completed
 * check that found nothing stays in Settings so the title bar never shows
 * permanent filler for a healthy, current install.
 */
function shouldShowEntry(state: UpdateState): boolean {
  if (state.status === "idle") return false;
  if (state.status === "error") return state.errorStage !== "check";
  // Startup checks stay invisible; only a re-check from a visible panel spins.
  if (state.status === "checking") return Boolean(state.checkedAt);
  return true;
}

function TriggerIcon({ state }: { state: UpdateState }) {
  if (state.status === "checking") return <ArrowClockwise className="is-spinning" aria-hidden="true" />;
  if (state.status === "downloading") return <CircleNotch className="is-spinning" aria-hidden="true" />;
  if (state.status === "ready") return <Package aria-hidden="true" />;
  if (state.status === "error") return <WarningCircle aria-hidden="true" />;
  return <DownloadSimple aria-hidden="true" />;
}

export function TitleBarUpdate({ state, locale, t, onCheck, onDownload, onInstall, onRestart, onDefer }: TitleBarUpdateProps) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState<Action>();
  const [confirmAction, setConfirmAction] = useState<ConfirmAction>();

  useEffect(() => {
    if (state.status === "idle") {
      setOpen(false);
      setConfirmAction(undefined);
    }
  }, [state.status]);

  if (!shouldShowEntry(state)) return null;

  async function run(action: Action, operation: () => Promise<unknown>) {
    setBusy(action);
    try {
      await operation();
    } finally {
      setBusy(undefined);
    }
  }

  function retry() {
    switch (state.errorStage) {
      case "download": return run("download", onDownload);
      case "install": return run("install", onInstall);
      case "restart": return run("restart", onRestart);
      default: return run("check", onCheck);
    }
  }

  async function confirmInstall() {
    setBusy("install");
    try {
      await onInstall();
      setConfirmAction(undefined);
    } finally {
      setBusy(undefined);
    }
  }

  async function confirmRestart() {
    setBusy("restart");
    try {
      await onRestart();
      setConfirmAction(undefined);
    } catch {
      // The updater state already carries the failure for inline retry.
    } finally {
      setBusy(undefined);
    }
  }

  const actionable = state.status === "available" || state.status === "ready" || state.status === "error";
  const isBusy = Boolean(busy) || state.status === "checking" || state.status === "downloading";
  const updateDate = formatUpdateDate(state.date, locale);
  const progress = state.contentLength && state.contentLength > 0
    ? Math.min(100, Math.round((state.downloadedBytes / state.contentLength) * 100))
    : undefined;
  const triggerLabel = state.status === "available" && state.version
    ? `${t("updateAvailable")} · v${state.version}`
    : state.status === "downloading" ? t("downloadingUpdate")
    : state.status === "ready" ? t("updateReady")
    : state.status === "error" ? updateErrorTitle(state.errorStage, t)
    : t("checkingForUpdates");

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger render={<button type="button" className={`titlebar-update-trigger${actionable ? " has-alert" : ""}`} aria-label={triggerLabel} title={triggerLabel} data-tauri-drag-region="false" />}>
        <TriggerIcon state={state} />
        {actionable && <span className="titlebar-update-dot" aria-hidden="true" />}
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Positioner side="bottom" align="end" sideOffset={8} className="titlebar-update-positioner">
          <Popover.Popup className="titlebar-update-popover" aria-label={t("appUpdates")}>
            <div className="titlebar-update-heading">
              <strong>{t("appUpdates")}</strong>
              <button
                type="button"
                className="titlebar-update-check"
                aria-label={t("checkForUpdates")}
                title={t("checkForUpdates")}
                disabled={isBusy || state.status === "ready"}
                onClick={() => void run("check", onCheck)}
              >
                <ArrowClockwise className={state.status === "checking" || busy === "check" ? "is-spinning" : undefined} aria-hidden="true" />
              </button>
            </div>

            <div className="settings-update-version">
              <div><span>{t("currentVersion")}</span><code>v{state.currentVersion}</code></div>
              {state.version && <div><span>{t("availableVersion")}</span><code>v{state.version}</code></div>}
              {updateDate && state.date && <div><span>{t("publishedOn")}</span><time dateTime={state.date}>{updateDate}</time></div>}
            </div>

            {(state.status === "available" || (state.status === "ready" && state.notes)) && (
              <div className="settings-update-notes">
                <span>{t("updateNotes")}</span>
                <p>{state.notes || t("noUpdateNotes")}</p>
              </div>
            )}

            {state.status === "available" && (
              <div className="settings-update-actions">
                <Button variant="quiet" disabled={isBusy} onClick={() => void onDefer()}>{t("updateLater")}</Button>
                <Button variant="primary" loading={busy === "download"} disabled={isBusy} onClick={() => void run("download", onDownload)}>{t("downloadUpdate")}</Button>
              </div>
            )}

            {state.status === "downloading" && (
              <div className="titlebar-update-status" role="status">
                <strong>{t("downloadingUpdate")}</strong>
                <span>{state.contentLength ? `${formatUpdateBytes(state.downloadedBytes)} / ${formatUpdateBytes(state.contentLength)}` : `${formatUpdateBytes(state.downloadedBytes)} · ${t("downloadProgressUnknown")}`}</span>
                {progress !== undefined ? <progress max={100} value={progress} aria-label={t("downloadProgress")} /> : <div className="settings-update-progress" aria-hidden="true" />}
              </div>
            )}

            {state.status === "ready" && !state.installed && (
              <div className="settings-update-actions">
                <Button variant="quiet" disabled={isBusy} onClick={() => void onDefer()}>{t("updateLater")}</Button>
                <Button variant="primary" disabled={isBusy} onClick={() => setConfirmAction("install")}>{t("installUpdate")}</Button>
              </div>
            )}

            {state.status === "ready" && state.installed && (
              <div className="titlebar-update-status" role="status">
                <strong>{t("updateInstalled")}</strong>
                <span>{state.restartRequired ? t("updateInstalledRestart") : t("updateInstalledWindows")}</span>
                {state.restartRequired && <Button variant="primary" disabled={isBusy} onClick={() => setConfirmAction("restart")}>{t("restartApp")}</Button>}
              </div>
            )}

            {state.status === "error" && (
              <div className="settings-update-status is-error" role="alert">
                <WarningCircle className="settings-update-icon" aria-hidden="true" />
                <div className="settings-update-copy"><strong>{updateErrorTitle(state.errorStage, t)}</strong><span>{state.error || t("unknownUpdateError")}</span></div>
                <Button variant="quiet" loading={busy !== undefined} onClick={() => void retry()}>{t("retry")}</Button>
              </div>
            )}
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>

      <ConfirmDialog open={confirmAction === "install"} title={t("confirmInstallUpdate")} body={t("confirmInstallUpdateHint")} confirmLabel={t("installUpdate")} cancelLabel={t("cancel")} busy={busy === "install"} onOpenChange={(next) => !next && busy !== "install" && setConfirmAction(undefined)} onConfirm={confirmInstall} />
      <ConfirmDialog open={confirmAction === "restart"} title={t("confirmRestartUpdate")} body={t("confirmRestartUpdateHint")} confirmLabel={t("restartApp")} cancelLabel={t("cancel")} busy={busy === "restart"} onOpenChange={(next) => !next && busy !== "restart" && setConfirmAction(undefined)} onConfirm={confirmRestart} />
    </Popover.Root>
  );
}
