import { Popover } from "@base-ui/react/popover";
import { ArrowClockwise, ArrowRight, ChartDonut } from "@phosphor-icons/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { AgentBrandIcon } from "../lib/brand-icons";
import { formatMoney, notifyUsageChanged, usageApi, type SubscriptionUsage } from "../lib/usage";
import { limitingWindow, providers, resetLabel, staleUsage, statusKeys, windowLabel, type UsageT } from "../lib/subscription-presentation";
import { Button } from "./ui/button";
import type { UsageNavigation } from "./UsageNavSettings";

export function QuotaLabel({account,now,t}:{account:SubscriptionUsage;now:number;t:UsageT}) {
  const w=limitingWindow(account);
  const stale=staleUsage(account,now);
  return <span className={`nav-quota-value${(w?.usedPercent??0)>=90?" is-critical":""}${stale?" is-stale":""}`}>
    {stale && w && <small aria-label={t("usageCached")}>~</small>}
    {w?.usedPercent!=null ? `${Math.round(100-w.usedPercent)}%` : w?.currency ? <>{formatMoney(w.remaining??w.used,w.currency)}{w.remaining==null && <small> {t("usageUsed")}</small>}</> : w?.unlimited ? t("usageUnlimited") : "—"}
  </span>;
}

/** Metadata-only first paint; provider I/O is throttled by Core and never gates boot. */
export function TitleBarUsage({t,onOpen,active,navigation}:{t:UsageT;onOpen:()=>void;active:boolean;navigation:UsageNavigation}) {
  const [accounts,setAccounts]=useState<SubscriptionUsage[]>([]);
  const [open,setOpen]=useState(false);
  const [pending,setPending]=useState(false);
  const [error,setError]=useState(false);
  const [now,setNow]=useState(Date.now());
  const rail=useRef<HTMLDivElement>(null);
  const live=useRef(false),flight=useRef(false),seq=useRef(0);
  const load=useCallback(async()=>{
    const ticket=++seq.current;
    try {const data=await usageApi.subscriptions();if(live.current && ticket===seq.current){setAccounts(data);setError(false);}return data;}
    catch {if(live.current)setError(true);return [];}
  },[]);
  const refresh=useCallback(async(force=false)=>{
    if(flight.current||document.hidden)return;
    flight.current=true;if(live.current)setPending(true);
    try {
      const data=await load();
      const queue=data.filter(s=>s.enabled);
      // Bound concurrency even when every provider is connected.
      await Promise.all(Array.from({length:Math.min(3,queue.length)},async()=>{
        while(live.current && !document.hidden && queue.length){const s=queue.shift()!;try{const updated=await usageApi.refresh(s.provider,force);notifyUsageChanged(updated.find(a=>a.provider===s.provider));}catch{if(live.current)setError(true);}}
      }));
    } finally {flight.current=false;if(live.current)setPending(false);}
  },[load]);
  useEffect(()=>{
    live.current=true;void load();
    const delayed=setTimeout(()=>void refresh(),1500);
    const timer=setInterval(()=>{if(!document.hidden){setNow(Date.now());void refresh();}},300_000);
    const visible=()=>{if(!document.hidden){setNow(Date.now());void refresh();}};
    const changed=()=>{void load();setNow(Date.now());};
    window.addEventListener("repoatlas:usage-updated",changed);
    document.addEventListener("visibilitychange",visible);
    return()=>{live.current=false;seq.current++;clearTimeout(delayed);clearInterval(timer);window.removeEventListener("repoatlas:usage-updated",changed);document.removeEventListener("visibilitychange",visible);};
  },[load,refresh]);
  useEffect(()=>{if(!open)return;setNow(Date.now());const timer=setInterval(()=>setNow(Date.now()),30000);return()=>clearInterval(timer);},[open]);
  const connected=accounts.filter(s=>s.enabled);
  const visible=connected.filter(s=>!navigation.hidden.includes(s.provider));
  const hasConnections=connected.length>0;
  useEffect(()=>{
    const element=rail.current;
    if(!element)return;
    const wheel=(event:WheelEvent)=>{
      if(event.ctrlKey || event.shiftKey || Math.abs(event.deltaX)>=Math.abs(event.deltaY) || element.scrollWidth<=element.clientWidth)return;
      event.preventDefault();
      element.scrollLeft+=event.deltaY*(event.deltaMode===1?16:event.deltaMode===2?element.clientWidth:1);
    };
    element.addEventListener("wheel",wheel,{passive:false});
    return()=>element.removeEventListener("wheel",wheel);
  },[hasConnections]);
  const manage=()=>{setOpen(false);onOpen();};
  if(!connected.length)return <button className="titlebar-ai-history titlebar-usage" aria-pressed={active} onClick={onOpen} title={error?t("usageReadFailed"):t("usageTitle")}><ChartDonut aria-hidden/><span>{t("usageTitle")}</span></button>;
  return <Popover.Root open={open} onOpenChange={setOpen}>
    <div ref={rail} className="titlebar-quota-rail" onKeyDown={event=>{
      const element=event.currentTarget;
      if((event.key!=="ArrowLeft" && event.key!=="ArrowRight") || element.scrollWidth<=element.clientWidth)return;
      event.preventDefault();element.scrollLeft+=event.key==="ArrowRight"?100:-100;
    }}>
    <Popover.Trigger className="titlebar-quota" aria-label={`${t("usageSubscriptions")} · ${visible.map(s=>`${providers[s.provider]?.name??s.provider} ${limitingWindow(s)?.usedPercent!=null?`${Math.round(100-limitingWindow(s)!.usedPercent!)}% ${t("usageRemaining")}`:t("usageDetails")}`).join(" · ")}`} title={t("usageSubscriptions")} data-active={active}>
      {!visible.length && <span className="nav-quota"><ChartDonut aria-hidden/><span>{t("usageTitle")}</span></span>}
      {visible.map(s=><span className="nav-quota" key={s.provider}><AgentBrandIcon agent={providers[s.provider]?.icon??s.provider}/><span className="nav-quota-name">{s.provider==="opencode-go"?"Go":providers[s.provider]?.name??s.provider}</span><QuotaLabel account={s} t={t} now={now}/></span>)}
    </Popover.Trigger>
    </div>
    <Popover.Portal><Popover.Positioner side="bottom" align="end" sideOffset={8} className="quota-popover-positioner"><Popover.Popup className="quota-popover">
      <header><Popover.Title>{t("usageSubscriptions")}</Popover.Title><Button variant="quiet" size="icon" loading={pending} aria-label={t("usageRefresh")} onClick={()=>void refresh(true)}><ArrowClockwise/></Button></header>
      <p className="quota-popover-hint">{t("usageNavHint")}</p>
      {error && <p role="alert">{t("usageReadFailed")}</p>}
      <div className="quota-popover-list">{connected.map(s=><section key={s.provider}><div className="quota-popover-brand"><AgentBrandIcon agent={providers[s.provider]?.icon??s.provider}/><strong>{providers[s.provider]?.name??s.provider}</strong><span>{s.plan}</span></div>
        {s.windows.map(w=><div className={`quota-popover-window${(w.usedPercent??0)>=90?" is-critical":(w.usedPercent??0)>=75?" is-warning":""}`} key={w.id}>
          <span>{windowLabel(w,t)}</span><strong>{w.usedPercent!=null?`${Math.round(100-w.usedPercent)}%`:w.currency?formatMoney(w.remaining??w.used,w.currency):w.unlimited?t("usageUnlimited"):"—"}</strong>
          {w.usedPercent!=null && !w.unlimited && <div className="quota-track" role="meter" aria-label={`${providers[s.provider]?.name} · ${windowLabel(w,t)} · ${t("usageUsed")}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={w.usedPercent}><span style={{width:`${w.usedPercent}%`}}/></div>}
          <small>{w.currency && `${t(w.remaining!=null?"usageBalance":"usageUsed")} · `}{resetLabel(w.resetsAt,now,t)}</small>
        </div>)}
        {staleUsage(s,now) && <p className="quota-stale">{t(s.status!=="ready"?statusKeys[s.status]??"usageUnavailable":"usageCached")}{s.fetchedAt && ` · ${new Date(s.fetchedAt).toLocaleTimeString([],{hour:"2-digit",minute:"2-digit"})}`}</p>}
      </section>)}</div>
      <footer className="quota-popover-footer">
        <Button variant="quiet" className="quota-manage" onClick={manage}>{t("usageManage")}<ArrowRight/></Button>
      </footer>
    </Popover.Popup></Popover.Positioner></Popover.Portal>
  </Popover.Root>;
}
