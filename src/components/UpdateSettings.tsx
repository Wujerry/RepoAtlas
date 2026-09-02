import { ArrowClockwise, CheckCircle, DownloadSimple, Package, WarningCircle } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "framer-motion";
import { useState } from "react";
import type { MessageKey } from "../i18n";
import { fadeMotion } from "../lib/motion";
import { formatLocaleTag } from "../lib/format";
import type { ToastTone, UpdateErrorStage, UpdateState } from "../types";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";

type Action = "check" | "download" | "install" | "restart";
type ConfirmAction = "install" | "restart";

export interface UpdateSettingsProps {
  state: UpdateState;
  locale?: string;
  t: (key: MessageKey) => string;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
  onCheck: () => Promise<UpdateState>;
  onDownload: () => Promise<UpdateState>;
  onInstall: () => Promise<boolean>;
  onRestart: () => Promise<void>;
  onDefer: () => Promise<void>;
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 1024) return `${Math.max(0, Math.round(bytes))} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`;
}

function formatDate(value?: string, locale?: string): string | undefined {
  if (!value) return undefined;
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return new Intl.DateTimeFormat(formatLocaleTag(locale), { dateStyle: "medium", timeStyle: "short" }).format(parsed);
}

function errorTitle(stage: UpdateErrorStage | undefined, t: (key: MessageKey) => string): string {
  switch (stage) {
    case "download": return t("updateDownloadFailed");
    case "install": return t("updateInstallFailed");
    case "restart": return t("updateRestartFailed");
    default: return t("updateCheckFailed");
  }
}

export function UpdateSettings({ state, locale, t, notify, onCheck, onDownload, onInstall, onRestart, onDefer }: UpdateSettingsProps) {
  const [busy, setBusy] = useState<Action>();
  const [confirmAction, setConfirmAction] = useState<ConfirmAction>();

  async function run(action: Action, operation: () => Promise<unknown>, failureKey: MessageKey) {
    setBusy(action);
    try {
      await operation();
    } catch (error) {
      notify("error", t(failureKey), String(error));
    } finally {
      setBusy(undefined);
    }
  }

  function check() {
    return run("check", onCheck, "updateCheckFailed");
  }

  function download() {
    return run("download", onDownload, "updateDownloadFailed");
  }

  async function confirmInstall() {
    setBusy("install");
    try {
      await onInstall();
      setConfirmAction(undefined);
    } catch (error) {
      notify("error", t("updateInstallFailed"), String(error));
    } finally {
      setBusy(undefined);
    }
  }

  async function confirmRestart() {
    setBusy("restart");
    try {
      await onRestart();
      setConfirmAction(undefined);
    } catch (error) {
      notify("error", t("updateRestartFailed"), String(error));
    } finally {
      setBusy(undefined);
    }
  }
  function retry() {
    switch (state.errorStage) {
      case "download": return download();
      case "install": return run("install", onInstall, "updateInstallFailed");
      case "restart": return run("restart", onRestart, "updateRestartFailed");
      default: return check();
    }
  }


  const checkedAt = formatDate(state.checkedAt, locale);
  const updateDate = formatDate(state.date, locale);
  const progress = state.contentLength && state.contentLength > 0
    ? Math.min(100, Math.round((state.downloadedBytes / state.contentLength) * 100))
    : undefined;
  const isBusy = Boolean(busy) || state.status === "checking" || state.status === "downloading";

  return (
    <section className="settings-card settings-updater" aria-busy={isBusy}>
      <div className="settings-card-heading">
        <div><p className="eyebrow">04</p><h2>{t("appUpdates")}</h2></div>
        <div className="settings-card-action"><p>{t("appUpdatesHint")}</p><Button variant="quiet" loading={busy === "check" || state.status === "checking"} disabled={isBusy} onClick={() => void check()}><ArrowClockwise aria-hidden="true" />{t("checkForUpdates")}</Button></div>
      </div>

      <div className="settings-update-version">
        <div><span>{t("currentVersion")}</span><code>v{state.currentVersion}</code></div>
        {state.version && <div><span>{t("availableVersion")}</span><code>v{state.version}</code></div>}
        {checkedAt && <div><span>{t("lastChecked")}</span><time dateTime={state.checkedAt}>{checkedAt}</time></div>}
      </div>

      <AnimatePresence mode="wait" initial={false}>
      {state.status === "checking" && <motion.div key="checking" className="settings-update-status" role="status" {...fadeMotion}><ArrowClockwise className="settings-update-icon is-spinning" aria-hidden="true" /><div><strong>{t("checkingForUpdates")}</strong><span>{t("offlineUpdateHint")}</span></div></motion.div>}

      {state.status === "idle" && <motion.div key="idle" className="settings-update-status" role="status" {...fadeMotion}><CheckCircle className="settings-update-icon" aria-hidden="true" /><div><strong>{state.deferred ? t("updateDeferred") : checkedAt ? t("upToDate") : t("updateNotChecked")}</strong><span>{state.deferred ? t("updateDeferredHint") : t("offlineUpdateHint")}</span></div></motion.div>}

      {state.status === "available" && <motion.div key="available" className="settings-update-status is-available" role="status" {...fadeMotion}><DownloadSimple className="settings-update-icon" aria-hidden="true" /><div className="settings-update-copy"><strong>{t("updateAvailable")}</strong><span>{updateDate ? `${t("publishedOn")} ${updateDate}` : t("signedUpdateHint")}</span>{state.notes ? <div className="settings-update-notes"><span>{t("updateNotes")}</span><p>{state.notes}</p></div> : <span>{t("noUpdateNotes")}</span>}</div><div className="settings-update-actions"><Button variant="primary" loading={busy === "download"} disabled={isBusy} onClick={() => void download()}>{t("downloadUpdate")}</Button><Button variant="quiet" disabled={isBusy} onClick={() => void onDefer()}>{t("updateLater")}</Button></div></motion.div>}

      {state.status === "downloading" && <motion.div key="downloading" className="settings-update-status" role="status" {...fadeMotion}><DownloadSimple className="settings-update-icon is-spinning" aria-hidden="true" /><div className="settings-update-copy"><strong>{t("downloadingUpdate")}</strong><span>{state.contentLength ? `${formatBytes(state.downloadedBytes)} / ${formatBytes(state.contentLength)}` : `${formatBytes(state.downloadedBytes)} · ${t("downloadProgressUnknown")}`}</span>{progress !== undefined ? <progress max="100" value={progress} aria-label={t("downloadProgress")}>{progress}%</progress> : <div className="settings-update-progress is-indeterminate" aria-hidden="true" />}</div></motion.div>}

      {state.status === "ready" && !state.installed && <motion.div key="ready" className="settings-update-status is-ready" role="status" {...fadeMotion}><Package className="settings-update-icon" aria-hidden="true" /><div className="settings-update-copy"><strong>{t("updateReady")}</strong><span>{t("installUpdateHint")}</span></div><div className="settings-update-actions"><Button variant="primary" disabled={isBusy} onClick={() => setConfirmAction("install")}>{t("installUpdate")}</Button><Button variant="quiet" disabled={isBusy} onClick={() => void onDefer()}>{t("updateLater")}</Button></div></motion.div>}

      {state.status === "ready" && state.installed && <motion.div key="installed" className="settings-update-status is-ready" role="status" {...fadeMotion}><CheckCircle className="settings-update-icon" aria-hidden="true" /><div className="settings-update-copy"><strong>{t("updateInstalled")}</strong><span>{state.restartRequired ? t("updateInstalledRestart") : t("updateInstalledWindows")}</span></div>{state.restartRequired && <Button variant="primary" disabled={isBusy} onClick={() => setConfirmAction("restart")}>{t("restartApp")}</Button>}</motion.div>}

      {state.status === "error" && <motion.div key="error" className="settings-update-status is-error" role="alert" {...fadeMotion}><WarningCircle className="settings-update-icon" aria-hidden="true" /><div className="settings-update-copy"><strong>{errorTitle(state.errorStage, t)}</strong><span>{state.error || t("unknownUpdateError")}</span></div><Button variant="quiet" disabled={isBusy} loading={busy === "check" || busy === "download" || busy === "install" || busy === "restart"} onClick={() => void retry()}>{t("retry")}</Button></motion.div>}
      </AnimatePresence>

      <ConfirmDialog open={confirmAction === "install"} title={t("confirmInstallUpdate")} body={t("confirmInstallUpdateHint")} confirmLabel={t("installUpdate")} cancelLabel={t("cancel")} busy={busy === "install"} onOpenChange={(open) => !open && busy !== "install" && setConfirmAction(undefined)} onConfirm={confirmInstall} />
      <ConfirmDialog open={confirmAction === "restart"} title={t("confirmRestartUpdate")} body={t("confirmRestartUpdateHint")} confirmLabel={t("restartApp")} cancelLabel={t("cancel")} busy={busy === "restart"} onOpenChange={(open) => !open && busy !== "restart" && setConfirmAction(undefined)} onConfirm={confirmRestart} />
    </section>
  );
}
