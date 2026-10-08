import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowsOutSimple, Bell, ChartDonut, MagnifyingGlass, Footprints, GearSix, Minus, Play, Plugs, Question, X } from "@phosphor-icons/react";

import type { ReactNode } from "react";
import { Brain } from "@phosphor-icons/react";
import { NavigationIcon } from "./ui/navigation-icon";
import { openSessionHistory } from "../lib/sessions";

export function TitleBar({ attentionOpen = false, tasksOpen = false, navigationLabels, usageEntry, usageLabel, usageOpen = false, onUsage, historyLabel = "Sessions", historyOpen = false, title, subtitle, commandLabel, minimizeLabel, maximizeLabel, closeLabel, helpLabel, settingsLabel, approvalsLabel, approvalCount, footprintsLabel, footprintsOpen = false, onFootprints, tasksLabel, tasksShortcut, tasksCount, onTasks, activeView, onCommand, onLibrary, onHelp, onSettings, onApprovals, updateEntry, portsLabel, portsOpen = false, onPorts }: { attentionOpen?: boolean; tasksOpen?: boolean; navigationLabels?: { tasks: string; attention: string; ports: string }; portsLabel?: string; portsOpen?: boolean; onPorts?: () => void; usageEntry?: ReactNode; usageLabel?: string; usageOpen?: boolean; onUsage?: () => void; historyLabel?: string; historyOpen?: boolean; title: string; subtitle: string; commandLabel: string; minimizeLabel: string; maximizeLabel: string; closeLabel: string; helpLabel: string; settingsLabel: string; approvalsLabel: string; approvalCount: number; footprintsLabel?: string; footprintsOpen?: boolean; onFootprints?: () => void; tasksLabel: string; tasksShortcut: string; tasksCount: number; onTasks: () => void; activeView: "library" | "settings" | "help"; onCommand: () => void; onLibrary: () => void; onHelp: () => void; onSettings: () => void; onApprovals: () => void; updateEntry?: ReactNode }) {
  const win = getCurrentWindow();
  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar-identity" data-tauri-drag-region>
        <button className={`brand ${activeView === "library" ? "active" : ""}`} onClick={onLibrary} data-tauri-drag-region="false">
          <img className="brand-mark" src="/repoatlas-mark.png" alt="" />
          <div>
            <div className="brand-title">{title}</div>
            <div className="brand-subtitle">{subtitle}</div>
          </div>
        </button>
        <div className="titlebar-pages titlebar-usage-area" data-tauri-drag-region="false">
          {usageEntry ?? (<button className="titlebar-ai-history titlebar-usage" aria-label={usageLabel ?? "Usage"} title={usageLabel} aria-pressed={usageOpen} onClick={onUsage}><NavigationIcon icon={ChartDonut} motion="usage" active={usageOpen} weight={usageOpen ? "fill" : "duotone"} /><span>{usageLabel ?? "Usage"}</span></button>)}
        </div>
      </div>
      <button className="titlebar-command" onClick={onCommand} data-tauri-drag-region="false"><MagnifyingGlass weight="regular" aria-hidden="true" /><span>{commandLabel}</span><kbd>{/mac/i.test(navigator.userAgent) ? "Cmd K" : "Ctrl K"}</kbd></button>
      <div className="titlebar-pages" data-tauri-drag-region="false">
        <div className="titlebar-tools" role="group" aria-label={`${historyLabel} / ${navigationLabels?.tasks ?? tasksLabel}`}>
          <button className="titlebar-ai-history" aria-pressed={historyOpen} onClick={() => openSessionHistory()} aria-label={historyLabel} title={historyLabel}><NavigationIcon icon={Brain} motion="sessions" active={historyOpen} weight={historyOpen ? "fill" : "duotone"} /><span>{historyLabel}</span></button>
          <button className={"titlebar-tasks has-tint" + (tasksCount > 0 ? " has-alert" : "")} aria-pressed={tasksOpen} onClick={onTasks} aria-label={`${tasksLabel}${tasksCount ? ` (${tasksCount})` : ""}`} aria-keyshortcuts="Control+`" title={`${tasksLabel} (${tasksShortcut})`}><NavigationIcon icon={Play} motion="tasks" active={tasksOpen} weight="fill" /><span className="nav-label">{navigationLabels?.tasks ?? tasksLabel}</span>{tasksCount > 0 && <span className="titlebar-badge" aria-hidden>{tasksCount > 9 ? "9+" : tasksCount}</span>}</button>
        </div>
        <span className="titlebar-divider" aria-hidden/>
        <div className="titlebar-tools" role="group" aria-label={title}>
          <button className="titlebar-port-processes" aria-pressed={portsOpen} onClick={onPorts} aria-label={portsLabel ?? "Ports & processes"} title={portsLabel ?? "Ports & processes"}><NavigationIcon icon={Plugs} motion="ports" active={portsOpen} weight={portsOpen ? "fill" : "regular"} /><span className="nav-label">{navigationLabels?.ports ?? portsLabel}</span></button>
          <button className="titlebar-footprints" aria-pressed={footprintsOpen} onClick={onFootprints} aria-label={footprintsLabel || "Footprints"} title={footprintsLabel || "Footprints"}><NavigationIcon icon={Footprints} motion="footprints" active={footprintsOpen} weight={footprintsOpen ? "duotone" : "regular"} /><span className="nav-label">{footprintsLabel}</span></button>
          <button className={approvalCount > 0 ? "has-alert" : ""} onClick={onApprovals} aria-pressed={attentionOpen} aria-label={`${approvalsLabel}${approvalCount ? ` (${approvalCount})` : ""}`} title={approvalsLabel}><NavigationIcon icon={Bell} motion="attention" active={attentionOpen} weight={approvalCount ? "fill" : "regular"} /><span className="nav-label">{navigationLabels?.attention ?? approvalsLabel}</span>{approvalCount > 0 && <span className="titlebar-badge" aria-hidden>{approvalCount > 9 ? "9+" : approvalCount}</span>}</button>
          <button className={activeView === "help" ? "active" : ""} onClick={onHelp} aria-label={helpLabel} aria-pressed={activeView === "help"} title={helpLabel}><NavigationIcon icon={Question} motion="help" active={activeView === "help"} /></button>
        </div>
        {updateEntry}
        <button className={activeView === "settings" ? "active" : ""} onClick={onSettings} aria-label={settingsLabel} aria-pressed={activeView === "settings"}><NavigationIcon icon={GearSix} motion="settings" active={activeView === "settings"} /></button>
      </div>
      <div className="window-controls">
        <button onClick={() => win.minimize()} aria-label={minimizeLabel}><Minus aria-hidden="true" /></button>
        <button onClick={() => win.toggleMaximize()} aria-label={maximizeLabel}><ArrowsOutSimple aria-hidden="true" /></button>
        <button className="close" onClick={() => win.close()} aria-label={closeLabel}><X aria-hidden="true" /></button>
      </div>
    </header>
  );
}
