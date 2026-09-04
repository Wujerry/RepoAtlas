import { open, save } from "@tauri-apps/plugin-dialog";
import { ArrowLeft } from "@phosphor-icons/react";
import { useState } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import type { AppSettings, ScanRoot, ToastTone, UpdateState } from "../types";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";
import { EmptyState } from "./ui/feedback";
import { UpdateSettings } from "./UpdateSettings";

export interface SettingsPaneProps {
  settings: AppSettings;
  scanRoots: ScanRoot[];
  scanning: boolean;
  t: (key: MessageKey) => string;
  notify: (tone: ToastTone, title: string, detail?: string) => void;
  onSettings: (settings: AppSettings) => Promise<void>;
  onAddRoot: () => Promise<void>;
  onRemoveRoot: (id: string) => Promise<void>;
  onScanRoot: (id: string) => Promise<void>;
  onReload: () => Promise<void>;
  updateState: UpdateState;
  onCheckForUpdates: () => Promise<UpdateState>;
  onDownloadUpdate: () => Promise<UpdateState>;
  onInstallUpdate: () => Promise<boolean>;
  onRestartApp: () => Promise<void>;
  onDeferUpdate: () => Promise<void>;
  onBack: () => void;
}

export function SettingsPane({
  settings,
  scanRoots,
  scanning,
  t,
  notify,
  onSettings,
  onAddRoot,
  onRemoveRoot,
  onScanRoot,
  onReload,
  updateState,
  onCheckForUpdates,
  onDownloadUpdate,
  onInstallUpdate,
  onRestartApp,
  onDeferUpdate,
  onBack,
}: SettingsPaneProps) {
  const [removingRoot, setRemovingRoot] = useState<string>();
  const [pendingRoot, setPendingRoot] = useState<ScanRoot>();
  const [dataBusy, setDataBusy] = useState<"export" | "import" | "backup">();
  const [pendingImportPath, setPendingImportPath] = useState<string>();

  async function exportData() {
    setDataBusy("export");
    try {
      const path = await save({
        defaultPath: "repoatlas-management-metadata.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (path) {
        await api.exportJsonTo(path);
        notify("success", t("exportComplete"), path);
      }
    } catch (error) {
      notify("error", t("exportFailed"), String(error));
    } finally {
      setDataBusy(undefined);
    }
  }

  async function importData() {
    try {
      const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
      if (typeof path === "string") {
        setPendingImportPath(path);
      }
    } catch (error) {
      notify("error", t("importFailed"), String(error));
    }
  }

  async function confirmImportData() {
    if (!pendingImportPath) return;
    setDataBusy("import");
    try {
      const count = await api.importJsonFrom(pendingImportPath);
      notify("success", t("importComplete"), `${count} ${t("projects")}`);
      setPendingImportPath(undefined);
      await onReload();
    } catch (error) {
      notify("error", t("importFailed"), String(error));
    } finally {
      setDataBusy(undefined);
    }
  }

  async function backupData() {
    setDataBusy("backup");
    try {
      const path = await save({
        defaultPath: "repoatlas-complete-backup.sqlite",
        filters: [{ name: "SQLite", extensions: ["sqlite", "db"] }],
      });
      if (path) {
        await api.backupDb(path);
        notify("success", t("backupComplete"), path);
      }
    } catch (error) {
      notify("error", t("backupFailed"), String(error));
    } finally {
      setDataBusy(undefined);
    }
  }

  return (
    <div className="settings-pane">
      <header className="settings-header">
        <Button type="button" className="settings-back" variant="quiet" onClick={(event) => { event.preventDefault(); event.stopPropagation(); onBack(); }}><ArrowLeft aria-hidden="true" />{t("backToProjects")}</Button>
        <p className="eyebrow">RepoAtlas</p>
        <h1 id="settings-page-title">{t("settings")}</h1>
        <p>{t("settingsIntro")}</p>
      </header>
      <div className="settings-content">
        <section className="settings-card">
          <div className="settings-card-heading">
            <div><p className="eyebrow">01</p><h2>{t("appearance")}</h2></div>
            <p>{t("appearanceHint")}</p>
          </div>
          <div className="settings-row">
            <div><strong>{t("theme")}</strong><span>{t("themeHint")}</span></div>
            <div className="segmented-control">
              {(["system", "light", "dark"] as const).map((value) => (
                <Button key={value} variant={settings.theme === value ? "primary" : "quiet"} aria-pressed={settings.theme === value} onClick={() => void onSettings({ ...settings, theme: value })}>
                  {t(value === "system" ? "followSystem" : value)}
                </Button>
              ))}
            </div>
          </div>
          <div className="settings-row">
            <div><strong>{t("language")}</strong><span>{t("languageHint")}</span></div>
            <div className="segmented-control">
              {(["system", "zh", "en"] as const).map((value) => (
                <Button key={value} variant={settings.locale === value ? "primary" : "quiet"} aria-pressed={settings.locale === value} onClick={() => void onSettings({ ...settings, locale: value })}>
                  {value === "system" ? t("followSystem") : value === "zh" ? "简体中文" : "English"}
                </Button>
              ))}
            </div>
          </div>
          <div className="settings-row">
            <div><strong>{t("uiFont")}</strong><span>{t("uiFontHint")}</span></div>
            <input className="field-control" defaultValue={settings.uiFont ?? ""} key={settings.uiFont ?? ""} placeholder={t("uiFontPlaceholder")} onBlur={(event) => { const uiFont = event.target.value.trim(); if (uiFont !== (settings.uiFont ?? "")) void onSettings({ ...settings, uiFont }); }} />
          </div>
          <div className="settings-row">
            <div><strong>{t("consoleFont")}</strong><span>{t("consoleFontHint")}</span></div>
            <input className="field-control" defaultValue={settings.consoleFont ?? ""} key={settings.consoleFont ?? ""} placeholder={t("consoleFontPlaceholder")} onBlur={(event) => { const consoleFont = event.target.value.trim(); if (consoleFont !== (settings.consoleFont ?? "")) void onSettings({ ...settings, consoleFont }); }} />
          </div>
        </section>

        <section className="settings-card">
          <div className="settings-card-heading">
            <div><p className="eyebrow">02</p><h2>{t("scanRoots")}</h2></div>
            <div className="settings-card-action"><p>{t("scanRootsHint")}</p><Button variant="primary" onClick={() => void onAddRoot()}>{t("addRoot")}</Button></div>
          </div>
          {scanRoots.length === 0 ? (
            <EmptyState compact title={t("noScanRoots")} body={t("noScanRootsHint")} actions={<Button variant="primary" onClick={() => void onAddRoot()}>{t("addRoot")}</Button>} />
          ) : (
            <div className="settings-list">
              {scanRoots.map((root) => (
                <div className="root-row" key={root.id}>
                  <div><code>{root.path}</code><span>{t("lastScanned")}: {root.lastScannedAt ?? "—"}</span></div>
                  <div><Button variant="quiet" disabled={scanning} onClick={() => void onScanRoot(root.id)}>{t("scan")}</Button><Button variant="danger" disabled={scanning} loading={removingRoot === root.id} onClick={() => setPendingRoot(root)}>{t("remove")}</Button></div>
                </div>
              ))}
            </div>
          )}
        </section>

        <UpdateSettings state={updateState} locale={settings.locale === "zh" || settings.locale === "en" ? settings.locale : undefined} t={t} notify={notify} onCheck={onCheckForUpdates} onDownload={onDownloadUpdate} onInstall={onInstallUpdate} onRestart={onRestartApp} onDefer={onDeferUpdate} />

        <section className="settings-card">
          <div className="settings-card-heading"><div><p className="eyebrow">04</p><h2>{t("dataAndBackup")}</h2></div><p>{t("dataHint")}</p></div>
          <div className="data-actions"><Button variant="primary" loading={dataBusy === "export"} disabled={Boolean(dataBusy)} onClick={() => void exportData()}>{t("exportData")}</Button><Button loading={dataBusy === "import"} disabled={Boolean(dataBusy)} onClick={() => void importData()}>{t("importData")}</Button><Button loading={dataBusy === "backup"} disabled={Boolean(dataBusy)} onClick={() => void backupData()}>{t("backupDatabase")}</Button></div>
        </section>
      </div>
      <ConfirmDialog open={Boolean(pendingRoot)} title={t("confirmRemoveRoot")} body={pendingRoot ? `${pendingRoot.path}\n${t("confirmRemoveRootHint")}` : ""} confirmLabel={t("remove")} cancelLabel={t("cancel")} busy={Boolean(removingRoot)} onOpenChange={(open) => !open && !removingRoot && setPendingRoot(undefined)} onConfirm={async () => { if (!pendingRoot) return; setRemovingRoot(pendingRoot.id); try { await onRemoveRoot(pendingRoot.id); setPendingRoot(undefined); } catch (error) { notify("error", t("removeRootFailed"), String(error)); } finally { setRemovingRoot(undefined); } }} />
      <ConfirmDialog open={Boolean(pendingImportPath)} title={t("confirmImportData")} body={pendingImportPath ? `${pendingImportPath}\n${t("confirmImportDataHint")}` : ""} confirmLabel={t("importData")} cancelLabel={t("cancel")} busy={dataBusy === "import"} onOpenChange={(open) => !open && dataBusy !== "import" && setPendingImportPath(undefined)} onConfirm={() => void confirmImportData()} />
    </div>
  );
}
