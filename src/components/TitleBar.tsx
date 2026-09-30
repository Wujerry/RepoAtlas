import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowsOutSimple, Bell, ChartDonut, MagnifyingGlass, Footprints, GearSix, Minus, Play, Question, X } from "@phosphor-icons/react";

import type { ReactNode } from "react";
import { Brain } from "@phosphor-icons/react";
import { openSessionHistory } from "../lib/sessions";

export function TitleBar({ usageEntry, usageLabel, usageOpen = false, onUsage, historyLabel = "Sessions", historyOpen = false, title, subtitle, commandLabel, minimizeLabel, maximizeLabel, closeLabel, helpLabel, settingsLabel, approvalsLabel, approvalCount, footprintsLabel, footprintsOpen = false, onFootprints, tasksLabel, tasksShortcut, tasksCount, onTasks, activeView, onCommand, onLibrary, onHelp, onSettings, onApprovals, updateEntry }: { usageEntry?: ReactNode; usageLabel?: string; usageOpen?: boolean; onUsage?: () => void; historyLabel?: string; historyOpen?: boolean; title: string; subtitle: string; commandLabel: string; minimizeLabel: string; maximizeLabel: string; closeLabel: string; helpLabel: string; settingsLabel: string; approvalsLabel: string; approvalCount: number; footprintsLabel?: string; footprintsOpen?: boolean; onFootprints?: () => void; tasksLabel: string; tasksShortcut: string; tasksCount: number; onTasks: () => void; activeView: "library" | "settings" | "help"; onCommand: () => void; onLibrary: () => void; onHelp: () => void; onSettings: () => void; onApprovals: () => void; updateEntry?: ReactNode }) {
  const win = getCurrentWindow();
  return (
    <header className="titlebar" data-tauri-drag-region>
      <button className={`brand ${activeView === "library" ? "active" : ""}`} onClick={onLibrary} data-tauri-drag-region="false">
        <img className="brand-mark" src="/repoatlas-mark.png" alt="" />
        <div>
          <div className="brand-title">{title}</div>
          <div className="brand-subtitle">{subtitle}</div>
        </div>
      </button>
      <button className="titlebar-command" onClick={onCommand} data-tauri-drag-region="false"><MagnifyingGlass weight="regular" aria-hidden="true" /><span>{commandLabel}</span><kbd>{/mac/i.test(navigator.userAgent) ? "Cmd K" : "Ctrl K"}</kbd></button>
      <div className="titlebar-pages" data-tauri-drag-region="false">
        {usageEntry ?? (<button className="titlebar-ai-history titlebar-usage" aria-label={usageLabel ?? "Usage"} title={usageLabel} aria-pressed={usageOpen} onClick={onUsage}><ChartDonut weight={usageOpen ? "fill" : "duotone"} aria-hidden /><span>{usageLabel ?? "Usage"}</span></button>)}
        <button className="titlebar-ai-history" aria-pressed={historyOpen} onClick={() => openSessionHistory()} aria-label={historyLabel} title={historyLabel}><Brain weight={historyOpen ? "fill" : "duotone"} aria-hidden="true" /><span>{historyLabel}</span></button>
        <span className="titlebar-divider" aria-hidden/>
        <div className="titlebar-tools" role="group" aria-label={title}>
          <button className={"has-tint" + (tasksCount > 0 ? " has-alert" : "")} onClick={onTasks} aria-label={`${tasksLabel}${tasksCount ? ` (${tasksCount})` : ""}`} aria-keyshortcuts="Control+`" title={`${tasksLabel} (${tasksShortcut})`}><Play weight="fill" aria-hidden/>{tasksCount > 0 && <span className="titlebar-badge" aria-hidden>{tasksCount > 9 ? "9+" : tasksCount}</span>}</button>
          <button className="titlebar-footprints" aria-pressed={footprintsOpen} onClick={onFootprints} aria-label={footprintsLabel || "Footprints"} title={footprintsLabel || "Footprints"}><Footprints weight={footprintsOpen ? "duotone" : "regular"} aria-hidden/></button>
          <button className={approvalCount > 0 ? "has-alert" : ""} onClick={onApprovals} aria-label={`${approvalsLabel}${approvalCount ? ` (${approvalCount})` : ""}`} title={approvalsLabel}><Bell weight={approvalCount ? "fill" : "regular"} aria-hidden/>{approvalCount > 0 && <span className="titlebar-badge" aria-hidden>{approvalCount > 9 ? "9+" : approvalCount}</span>}</button>
          <button className={activeView === "help" ? "active" : ""} onClick={onHelp} aria-label={helpLabel} aria-pressed={activeView === "help"} title={helpLabel}><Question aria-hidden/></button>
        </div>
        {updateEntry}
        <button className={activeView === "settings" ? "active" : ""} onClick={onSettings} aria-label={settingsLabel} aria-pressed={activeView === "settings"}><GearSix aria-hidden="true" /></button>
      </div>
      <div className="window-controls">
        <button onClick={() => win.minimize()} aria-label={minimizeLabel}><Minus aria-hidden="true" /></button>
        <button onClick={() => win.toggleMaximize()} aria-label={maximizeLabel}><ArrowsOutSimple aria-hidden="true" /></button>
        <button className="close" onClick={() => win.close()} aria-label={closeLabel}><X aria-hidden="true" /></button>
      </div>
    </header>
  );
}
