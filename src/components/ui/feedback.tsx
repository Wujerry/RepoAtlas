import { CheckCircle, Copy, Info, MapTrifold, Warning, WarningCircle, X } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import type { ToastMessage } from "../../types";
import { Button } from "./button";

export function Skeleton({ className = "" }: { className?: string }) {
  return <span className={`skeleton ${className}`} aria-hidden="true" />;
}

export function EmptyState({ title, body, actions, compact = false }: { title: string; body: string; actions?: ReactNode; compact?: boolean }) {
  return (
    <div className={`empty-state ${compact ? "empty-state-compact" : ""}`}>
      <span className="empty-state-mark" aria-hidden="true"><MapTrifold /></span>
      <h2>{title}</h2>
      <p>{body}</p>
      {actions && <div className="empty-state-actions">{actions}</div>}
    </div>
  );
}

const icons = { info: Info, success: CheckCircle, warning: Warning, error: WarningCircle };

export function ToastViewport({ toasts, onDismiss }: { toasts: ToastMessage[]; onDismiss: (id: string) => void }) {
  return (
    <div className="toast-viewport" aria-live="polite" aria-relevant="additions removals">
      {toasts.map((toast) => {
        const Icon = icons[toast.tone];
        return (
          <article className={`toast toast-${toast.tone}`} key={toast.id} role={toast.tone === "error" ? "alert" : "status"}>
            <Icon className="toast-icon" weight="duotone" aria-hidden="true" />
            <div className="toast-copy"><strong>{toast.title}</strong>{toast.detail && <p>{toast.detail}</p>}</div>
            <div className="toast-actions">
              {toast.detail && <Button variant="quiet" size="icon" aria-label="复制详情 / Copy details" onClick={() => void navigator.clipboard.writeText(toast.detail ?? "")}><Copy aria-hidden="true" /></Button>}
              <Button variant="quiet" size="icon" aria-label="关闭 / Dismiss" onClick={() => onDismiss(toast.id)}><X aria-hidden="true" /></Button>
            </div>
          </article>
        );
      })}
    </div>
  );
}

export function AppSkeleton() {
  return (
    <main id="main-content" className="main-pane skeleton-page" aria-label="Loading RepoAtlas">
      <div className="skeleton-hero"><Skeleton className="skeleton-title" /><Skeleton className="skeleton-line" /></div>
      <div className="skeleton-grid"><Skeleton className="skeleton-card" /><Skeleton className="skeleton-card" /><Skeleton className="skeleton-card skeleton-wide" /></div>
    </main>
  );
}

export function WorkspaceSkeleton() {
  return <div className="workspace-skeleton"><AppSkeleton /></div>;
}
