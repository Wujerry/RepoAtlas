import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowsOutSimple, Bell, Command, GearSix, Minus, Play, Question, X } from "@phosphor-icons/react";

export function TitleBar({ title, subtitle, commandLabel, minimizeLabel, maximizeLabel, closeLabel, helpLabel, settingsLabel, approvalsLabel, approvalCount, tasksLabel, tasksShortcut, tasksCount, onTasks, activeView, onCommand, onLibrary, onHelp, onSettings, onApprovals }: { title: string; subtitle: string; commandLabel: string; minimizeLabel: string; maximizeLabel: string; closeLabel: string; helpLabel: string; settingsLabel: string; approvalsLabel: string; approvalCount: number; tasksLabel: string; tasksShortcut: string; tasksCount: number; onTasks: () => void; activeView: "library" | "settings" | "help"; onCommand: () => void; onLibrary: () => void; onHelp: () => void; onSettings: () => void; onApprovals: () => void }) {
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
      <button className="titlebar-command" onClick={onCommand} data-tauri-drag-region="false"><Command weight="bold" aria-hidden="true" /><span>{commandLabel}</span><kbd>Ctrl K</kbd></button>
      <div className="titlebar-pages" data-tauri-drag-region="false">
        <button className={"has-tint" + (tasksCount > 0 ? " has-alert" : "")} onClick={onTasks} aria-label={`${tasksLabel}${tasksCount ? ` (${tasksCount})` : ""}`} aria-keyshortcuts={"Control+`"} title={`${tasksLabel} (${tasksShortcut})`}><Play weight="fill" aria-hidden="true" />{tasksCount > 0 && <span className="titlebar-badge" aria-hidden="true">{tasksCount > 9 ? "9+" : tasksCount}</span>}</button>
        <button className={approvalCount > 0 ? "has-alert" : ""} onClick={onApprovals} aria-label={`${approvalsLabel}${approvalCount ? ` (${approvalCount})` : ""}`}><Bell weight={approvalCount ? "fill" : "regular"} aria-hidden="true" />{approvalCount > 0 && <span className="titlebar-badge" aria-hidden="true">{approvalCount > 9 ? "9+" : approvalCount}</span>}</button>
        <button className={activeView === "help" ? "active" : ""} onClick={onHelp} aria-label={helpLabel} aria-pressed={activeView === "help"}><Question aria-hidden="true" /></button>
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
