import { Dialog } from "@base-ui/react/dialog";
import { ArrowClockwise, ArrowRight, ChartDonut, Clock, Plus, X } from "@phosphor-icons/react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { MessageKey } from "../i18n";
import { AgentBrandIcon } from "../lib/brand-icons";
import { sessionAgentName, sessionApi } from "../lib/sessions";
import { formatMoney, formatTokens, notifyUsageChanged, usageApi, type AgentUsageSummary, type SubscriptionUsage, type UsageWindow } from "../lib/usage";
import { providers, resetLabel, statusKeys, windowLabel } from "../lib/subscription-presentation";
export { resetLabel } from "../lib/subscription-presentation";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";
import { UsageNavFeedback, UsageNavSwitch, type UsageNavigation } from "./UsageNavSettings";

type T = (key: MessageKey) => string;
export function QuotaMeter({ window:w, t, now }: {window:UsageWindow;t:T;now:number}) {
  const percent=w.usedPercent;
  const expired=!!w.resetsAt && Date.parse(w.resetsAt)<=now;
  const severity=percent != null && percent>=90 ? " is-critical" : percent != null && percent>=75 ? " is-warning" : "";
  return <div className={`quota-window${severity}${expired ? " is-expired" : ""}`}>
    <div className="quota-window-heading"><span>{windowLabel(w,t)}{w.currency && <small> · {t(w.remaining!=null?"usageBalance":"usageUsed")}</small>}</span><strong>{w.unlimited ? t("usageUnlimited") : percent == null ? (w.currency ? formatMoney(w.remaining ?? w.used,w.currency) : "—") : <>{Math.round(Math.max(0,100-percent))}<small>% {t("usageRemaining")}</small></>}</strong></div>
    {!w.unlimited && percent != null && <div className="quota-track" role="meter" aria-label={`${windowLabel(w,t)} · ${t("usageUsed")}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent ?? undefined} aria-valuetext={percent == null ? "—" : `${percent}%`}><span style={{width:`${percent ?? 0}%`}} /></div>}
    <div className="quota-reset"><Clock size={12} aria-hidden /><time dateTime={w.resetsAt ?? undefined} title={w.resetsAt ? new Date(w.resetsAt).toLocaleString() : undefined}>{resetLabel(w.resetsAt,now,t)}</time>{w.used != null && w.limit != null && <span>{w.currency?formatMoney(w.used,w.currency):w.used.toLocaleString()} / {w.currency?formatMoney(w.limit,w.currency):w.limit.toLocaleString()}</span>}</div>
  </div>;
}

export default function UsagePanel({t,onClose,navigation}:{t:T;onClose:()=>void;navigation:UsageNavigation}) {
  const [subscriptions,setSubscriptions]=useState<SubscriptionUsage[]>([]);
  const [summary,setSummary]=useState<AgentUsageSummary[]>([]);
  const [tab,setTab]=useState("subscriptions");
  const [loading,setLoading]=useState(true);
  const [pending,setPending]=useState<Set<string>>(new Set());
  const [error,setError]=useState("");
  const [connect,setConnect]=useState<string>();
  const [connecting,setConnecting]=useState(false);
  const [historyBusy,setHistoryBusy]=useState(false);
  const [now,setNow]=useState(Date.now());
  const alive=useRef(true);
  const pendingRef=useRef(new Set<string>());
  const refresh=useCallback(async (provider:string,force=false) => {
    if(pendingRef.current.has(provider))return;
    pendingRef.current.add(provider); setPending(new Set(pendingRef.current));
    try {
      const result=await usageApi.refresh(provider,force);
      const updated=result.find(item=>item.provider===provider);
      notifyUsageChanged(updated);
      if(alive.current && updated)setSubscriptions(items=>items.map(item=>item.provider===provider?updated:item));
    } catch {if(alive.current)setError(t("usageReadFailed"));}
    finally {pendingRef.current.delete(provider);if(alive.current)setPending(new Set(pendingRef.current));}
  },[t]);
  const refreshAll=useCallback(async(accounts:SubscriptionUsage[],force=false)=>{
    const queue=accounts.filter(s=>s.enabled);
    await Promise.all(Array.from({length:Math.min(3,queue.length)},async()=>{
      while(alive.current && !document.hidden && queue.length)await refresh(queue.shift()!.provider,force);
    }));
  },[refresh]);
  useEffect(()=>{
    alive.current=true;
    let current=true;
    void Promise.allSettled([usageApi.subscriptions(),usageApi.summary()]).then(([accounts,history])=>{
      if(!alive.current||!current)return;
      if(accounts.status==="fulfilled") {setSubscriptions(accounts.value);void refreshAll(accounts.value);}
      if(history.status==="fulfilled")setSummary(history.value);
      if(accounts.status==="rejected"||history.status==="rejected")setError(t("usageReadFailed"));
      setLoading(false);
    });
    const timer=setInterval(()=>{if(!document.hidden)setNow(Date.now());},30_000);
    return ()=>{current=false;alive.current=false;clearInterval(timer);};
  },[refreshAll,t]);
  useEffect(()=>{
    let current=true,revision=0;
    const changed=(event:Event)=>{
      const ticket=++revision;
      const account=(event as CustomEvent<SubscriptionUsage|undefined>).detail;
      if(account){setSubscriptions(items=>items.map(item=>item.provider===account.provider?account:item));return;}
      void usageApi.subscriptions().then(items=>{if(current && ticket===revision)setSubscriptions(items);}).catch(()=>{if(current)setError(t("usageReadFailed"));});
    };
    window.addEventListener("repoatlas:usage-updated",changed);
    return()=>{current=false;window.removeEventListener("repoatlas:usage-updated",changed);};
  },[t]);
  useEffect(()=>{
    if(!historyBusy)return;
    let current=true;let timer:ReturnType<typeof setTimeout>;
  const poll=async()=>{
      if(document.hidden){timer=setTimeout(()=>void poll(),1200);return;}
      try {const job=await sessionApi.status();if(!current)return;
        if(job.running){timer=setTimeout(()=>void poll(),1200);return;}
        const updated=await usageApi.summary();if(!current)return;
        setSummary(updated);setHistoryBusy(false);window.dispatchEvent(new Event("repoatlas:sessions-updated"));
        if(job.errors)setError(t("ahRefreshFailed"));
      }catch{if(current){setHistoryBusy(false);setError(t("usageReadFailed"));}}
    };
    timer=setTimeout(()=>void poll(),1000);
    return()=>{current=false;clearTimeout(timer);};
  },[historyBusy,t]);
  const measured=summary.reduce((n,s)=>n+s.measuredSessions,0);
  const sessions=summary.reduce((n,s)=>n+s.sessions,0);
  const total=summary.reduce((n,s)=>n+(s.totalTokens??0),0);
  async function changeConnection(provider:string,enabled:boolean) {
    setConnecting(true);setError("");
    try {await usageApi.enable(provider,enabled);const result=await usageApi.subscriptions();if(!alive.current)return;setSubscriptions(result);notifyUsageChanged();setConnect(undefined);if(enabled)void refresh(provider);}
    catch{if(alive.current)setError(t("usageReadFailed"));}
    finally{if(alive.current)setConnecting(false);}
  }
  const connected=subscriptions.filter(s=>s.enabled);
  const available=subscriptions.filter(s=>!s.enabled);
  const estimated=summary.some(s=>s.estimatedUsd!=null)?summary.reduce((n,s)=>n+(s.estimatedUsd??0),0):null;
  const priced=summary.reduce((n,s)=>n+(s.pricedTokens??0),0);
  return <Dialog.Root modal={false} open onOpenChange={(open,details)=>{if(!open && details.reason!=="outside-press" && details.reason!=="focus-out")onClose();}}><Dialog.Portal><Dialog.Popup className="usage-dialog" aria-describedby={undefined}>
    <header className="usage-header"><Dialog.Title><ChartDonut size={20} weight="duotone" aria-hidden/>{t("usageTitle")}</Dialog.Title><Button size="icon" variant="quiet" aria-label={t("close")} onClick={onClose}><X/></Button></header>
    <div className="usage-scroll">
      <div className="usage-intro"><div><h2>{t("usageHeading")}</h2><p>{t("usageSubtitle")}</p></div><span className="usage-connected-count">{connected.length} {t("usageConnected")}</span></div>
      <div className="usage-navigation"><div role="group" aria-label={t("usageTitle")}><button aria-pressed={tab==="subscriptions"} onClick={()=>setTab("subscriptions")}>{t("usageSubscriptions")}</button><button aria-pressed={tab==="history"} onClick={()=>setTab("history")}>{t("usageHistory")}</button></div>
        {tab==="subscriptions" ? <Button variant="quiet" loading={pending.size>0} disabled={!connected.length} onClick={()=>{setError("");void refreshAll(connected,true);}}><ArrowClockwise/>{t("usageRefresh")}</Button> : <Button variant="quiet" loading={historyBusy} onClick={()=>{setError("");setHistoryBusy(true);void sessionApi.refresh(true).catch(()=>{setHistoryBusy(false);setError(t("usageReadFailed"));});}}><ArrowClockwise/>{t("usageRefreshHistory")}</Button>}
      </div>
      {error && <div role="alert" className="usage-error">{error}<Button variant="quiet" onClick={()=>{setError("");void Promise.all([usageApi.subscriptions(),usageApi.summary()]).then(([s,h])=>{if(alive.current){setSubscriptions(s);setSummary(h);}}).catch(()=>setError(t("usageReadFailed")));}}>{t("retry")}</Button></div>}
      {loading && <div className="usage-skeleton" role="status">{t("loading")}</div>}
      {historyBusy && <p role="status" className="usage-note">{t("usageRefreshingHistory")}</p>}
      {tab==="subscriptions" ? <>
        <div className="usage-display-note"><UsageNavFeedback navigation={navigation} t={t}/></div>
        <section className="subscription-list" aria-label={t("usageSubscriptions")}>
          {connected.map(s=><article className="subscription-row" key={s.provider}>
            <header><span className="subscription-brand"><AgentBrandIcon agent={providers[s.provider]?.icon??s.provider}/></span><div><h3>{providers[s.provider]?.name??s.provider}</h3><span>{s.plan??t(providers[s.provider]?.kind==="balance"?"usageApiBalance":"usageUnknownPlan")}</span></div><Button variant="quiet" size="icon" loading={pending.has(s.provider)} aria-label={`${t("usageRefresh")} · ${providers[s.provider]?.name}`} onClick={()=>void refresh(s.provider,true)}><ArrowClockwise/></Button></header>
            <div className="subscription-windows">{s.windows.map(w=><QuotaMeter key={w.id} window={w} now={now} t={t}/>)}
              {!s.windows.length && <p className="subscription-status" role="status">{t(statusKeys[s.status]??"usageUnavailable")}</p>}
            </div>
            <div className="subscription-visibility"><UsageNavSwitch provider={s.provider} navigation={navigation} t={t}/></div>
            <footer><span>{s.status!=="ready" && s.windows.length>0 && <span className="quota-stale">{t(statusKeys[s.status]??"usageUnavailable")} · {t("usageCached")} · </span>}{s.fetchedAt ? <>{t("usageUpdated")} <time dateTime={s.fetchedAt} title={new Date(s.fetchedAt).toLocaleString()}>{new Date(s.fetchedAt).toLocaleTimeString([], {hour:"2-digit",minute:"2-digit"})}</time></> : t("usageNeverUpdated")}</span><details className="subscription-settings"><summary>{t("usageConnection")}</summary><div><code>{providers[s.provider]?.connection}</code><Button variant="quiet" disabled={pending.has(s.provider)||connecting} onClick={()=>void changeConnection(s.provider,false)}>{t("usageDisconnect")}</Button></div></details></footer>
          </article>)}
        </section>
        {!loading && available.length>0 && <details className="provider-catalog" open={connected.length===0?true:undefined}>
          <summary><Plus size={16}/><strong>{t("usageAddProvider")}</strong><span>{available.length} {t("usageAvailable")}</span></summary>
          <p>{t("usageConnectionHint")}</p><div className="provider-catalog-grid">{available.map(s=><div className="provider-option" key={s.provider}><AgentBrandIcon agent={providers[s.provider]?.icon??s.provider}/><div><strong>{providers[s.provider]?.name??s.provider}</strong><small>{t(providers[s.provider]?.kind==="balance"?"usageApiBalance":"usageSubscriptions")}</small></div><Button variant="quiet" onClick={()=>setConnect(s.provider)} aria-label={`${t("usageConnect")} · ${providers[s.provider]?.name??s.provider}`}><Plus/>{t("usageConnect")}</Button></div>)}</div>
        </details>}
        <p className="usage-note usage-provider-note">{t("usageProviderCoverage")}</p>
      </> : <section className="usage-history" aria-label={t("usageHistory")}>
        <div className="usage-stat-strip"><div><span>{t("usageRecorded")}</span><strong>{formatTokens(measured?total:null)}</strong><small>{measured.toLocaleString()} / {sessions.toLocaleString()} {t("usageSessionCount")}</small></div><div><span>{t("usageEquivalent")}</span><strong>{estimated==null?"—":`≈ ${formatMoney(estimated)}`}</strong><small>{t("usagePriceCoverage")} {total?Math.round(priced/total*100):0}%</small></div></div>
        <p className="usage-note">{t("usagePriceHint")}</p>
        {!summary.length && <div className="usage-empty"><ChartDonut size={36}/><h3>{t("usageNoHistory")}</h3><p>{t("usageNoHistoryHint")}</p></div>}
        {[...summary].sort((a,b)=>(b.totalTokens??0)-(a.totalTokens??0)).map(s=><div className="usage-agent-row" key={s.adapter}><AgentBrandIcon agent={s.adapter}/><div><strong>{sessionAgentName(s.adapter)}</strong><small>{s.measuredSessions} / {s.sessions} {t("usageSessionCount")}</small></div><div className="usage-agent-bar" aria-hidden><span style={{width:`${total && s.totalTokens?Math.max(1,s.totalTokens/total*100):0}%`}}/></div><strong>{formatTokens(s.totalTokens)}<small> tokens</small></strong><div className="usage-agent-cost"><strong>{s.estimatedUsd==null?"—":`≈ ${formatMoney(s.estimatedUsd)}`}</strong><small>{t("usagePriceCoverage")} {s.totalTokens?Math.round((s.pricedTokens??0)/s.totalTokens*100):0}%</small></div></div>)}
        <p className="usage-note">{t("usagePriceDate")} 2026-09-30 · {t("usageHistoryHint")}</p>
      </section>}
      <footer className="usage-bottom"><span>{t("usageHistoryHint")}</span><Button variant="quiet" onClick={()=>{onClose();window.dispatchEvent(new CustomEvent("repoatlas:session-history",{detail:{sources:true}}));}}>{t("usageHistorySources")}<ArrowRight/></Button></footer>
    </div>
    <ConfirmDialog className="ah-confirm" open={!!connect} title={`${t("usageConnectTitle")} · ${providers[connect??""]?.name??""}`} body={<>{t(connect==="antigravity"?"usageAntigravityConnectHint":"usageConnectHint")}<code className="usage-connection-path">{providers[connect??""]?.connection}{connect==="claude" && <><br/>{t("usageCredentialStore")}</>}</code></>} confirmVariant="primary" confirmLabel={t("usageConnectAction")} cancelLabel={t("cancel")} busy={connecting} onOpenChange={open=>{if(!open&&!connecting)setConnect(undefined);}} onConfirm={()=>connect?changeConnection(connect,true):undefined}/>
  </Dialog.Popup></Dialog.Portal></Dialog.Root>;
}
