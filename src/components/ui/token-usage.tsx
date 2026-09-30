import { ChartDonut } from "@phosphor-icons/react";
import type { MessageKey } from "../../i18n";
import { formatMoney, formatTokens, type TokenUsage } from "../../lib/usage";
import { DetailPopover } from "./detail-popover";
type T = (key: MessageKey) => string;
export function TokenBadge({ usage, t }: { usage?: TokenUsage | null; t: T }) {
  return <span className={`token-badge${usage ? "" : " is-unknown"}`} title={usage ? `${usage.totalTokens.toLocaleString()} tokens · ${t(usage.partial ? "usagePartial" : "usageRecorded")}` : t("usageMissing")}>
    <ChartDonut size={13} aria-hidden /><span>{formatTokens(usage?.totalTokens)}</span><small>tokens</small>
    {!!usage?.cost?.pricedTokens && <span className="token-cost" title={`${t("usageEquivalent")} · ${t("usagePriceHint")}${usage.cost.unpricedTokens ? ` · ${t("usagePricePartial")}` : ""}`}>≈ {formatMoney(usage.cost.usd)}{usage.cost.unpricedTokens>0 && <small> · {t("usagePricePartialShort")}</small>}</span>}
  </span>;
}
export function TokenBreakdown({ usage, t, floating = false }: { usage?: TokenUsage | null; t: T; floating?: boolean }) {
  if (!usage) return <p className="token-missing">{t("usageMissing")}</p>;
  const fraction = usage.totalTokens ? Math.min(100, (usage.inputTokens ?? 0) / usage.totalTokens * 100) : 0;
  const body = <div className="token-breakdown-body">
    <div className="token-composition" aria-hidden><span style={{ width: `${fraction}%` }} /></div>
    <dl>{([
      ["usageInput", usage.inputTokens], ["usageOutput", usage.outputTokens],
      ["usageCacheRead", usage.cacheReadTokens], ["usageCacheWrite", usage.cacheWriteTokens], ["usageReasoning", usage.reasoningTokens],
    ] as const).map(([label, count]) => <div key={label}><dt>{t(label)}</dt><dd>{count == null ? "—" : count.toLocaleString()}</dd></div>)}</dl>
    {usage.model && <p className="token-model">{usage.model}</p>}<p>{t("usageSubsetHint")}{usage.partial && ` ${t("usagePartial")}`}</p>
    <div className="token-cost-details"><strong>{t("usageEquivalent")} · {usage.cost?.pricedTokens ? `≈ ${formatMoney(usage.cost.usd)}` : "—"}</strong><p>{t(usage.cost?.pricedTokens ? "usagePriceHint" : "usagePriceMissing")}</p>{usage.cost && <p>{t("usagePriceCoverage")} {usage.totalTokens ? Math.round(usage.cost.pricedTokens/usage.totalTokens*100) : 0}% · {t("usagePriceDate")} {usage.cost.priceDate}{usage.cost.assumptions && ` · ${t("usagePriceAssumptions")}`}</p>}</div>
  </div>;
  if (floating) return <div className="token-breakdown token-breakdown-floating"><TokenBadge usage={usage} t={t}/><DetailPopover title={t("usageDetails")} closeLabel={t("close")}><div className="token-breakdown">{body}</div></DetailPopover></div>;
  return <details className="token-breakdown"><summary><TokenBadge usage={usage} t={t}/><span>{t("usageDetails")}</span></summary>{body}</details>;
}
