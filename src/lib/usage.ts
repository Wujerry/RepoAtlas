import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "../types";
export interface TokenUsage {
  totalTokens: number; inputTokens: number | null; outputTokens: number | null;
  cacheReadTokens: number | null; cacheWriteTokens: number | null; reasoningTokens: number | null;
  model: string | null; partial: boolean;
  cost?: { usd: number; pricedTokens: number; unpricedTokens: number; priceDate: string; assumptions: boolean } | null;
}
export interface UsageWindow { id: string; label: string; usedPercent: number | null; used: number | null; limit: number | null; unlimited: boolean; resetsAt: string | null; windowMinutes: number | null; remaining?: number | null; currency?: string | null }
export interface SubscriptionUsage { provider: string; enabled: boolean; plan: string | null; status: string; fetchedAt: string | null; attemptedAt: string | null; windows: UsageWindow[] }
export interface AgentUsageSummary { adapter: string; sessions: number; measuredSessions: number; totalTokens: number | null; estimatedUsd?: number | null; pricedTokens?: number }
export const usageApi = {
  setNavigation: (hidden: string[]) => invoke<AppSettings>("set_usage_navigation", { hidden }),
  subscriptions: () => invoke<SubscriptionUsage[]>("subscription_usage"),
  enable: (provider: string, enabled: boolean) => invoke<void>("set_subscription_enabled", { provider, enabled }),
  refresh: (provider: string, force = false) => invoke<SubscriptionUsage[]>("refresh_subscription_usage", { provider, force }),
  summary: () => invoke<AgentUsageSummary[]>("session_usage_summary"),
};
export function formatMoney(value: number | null | undefined, currency = "USD") {
  if (value == null || !Number.isFinite(value) || value < 0) return "—";
  if (value > 0 && value < .01) return currency === "USD" ? "<$0.01" : "<¥0.01";
  return new Intl.NumberFormat("en-US", { style: "currency", currency, maximumFractionDigits: 2 }).format(value);
}
export function notifyUsageChanged(account?: SubscriptionUsage) { window.dispatchEvent(new CustomEvent("repoatlas:usage-updated", {detail:account})); }
export function formatTokens(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value) || value < 0) return "—";
  if (value >= 1_000_000_000) return `${Number((value / 1_000_000_000).toFixed(2))}B`;
  if (value >= 1_000_000) return `${Number((value / 1_000_000).toFixed(2))}M`;
  if (value >= 1000) return `${Number((value / 1000).toFixed(1))}K`;
  return Math.round(value).toLocaleString();
}
