import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { dictionaries, type MessageKey } from "../i18n";
import { formatMoney, formatTokens, notifyUsageChanged, type SubscriptionUsage, type TokenUsage } from "../lib/usage";
import { TitleBarUsage } from "../components/TitleBarUsage";
import { useUsageNavigation, UsageNavFeedback, UsageNavSwitch, type UsageNavigation } from "../components/UsageNavSettings";
import { staleUsage, windowLabel } from "../lib/subscription-presentation";
import { TokenBadge, TokenBreakdown } from "../components/ui/token-usage";
import UsagePanel, { QuotaMeter, resetLabel } from "../components/UsagePanel";
const mocks=vi.hoisted(()=>({subscriptions:vi.fn(),summary:vi.fn(),enable:vi.fn(),refresh:vi.fn()}));
vi.mock("../lib/usage",async original=>({...await original<typeof import("../lib/usage")>(),usageApi:mocks}));
const navigation: UsageNavigation = {hidden: [], status: "idle", setVisible: vi.fn(async () => {})};
const t=(key:MessageKey)=>dictionaries.en[key];
const account=(provider:string,enabled=true):SubscriptionUsage=>({provider,enabled,plan:"Plus",status:enabled?"ready":"disabled",fetchedAt:"2026-09-30T08:00:00Z",attemptedAt:"2026-09-30T08:00:00Z",windows:[{id:"weekly",label:"weekly",usedPercent:25,used:null,limit:null,unlimited:false,resetsAt:"2026-10-01T08:00:00Z",windowMinutes:10080}]});
beforeEach(()=>{vi.clearAllMocks();mocks.subscriptions.mockResolvedValue([account("codex",false)]);mocks.summary.mockResolvedValue([]);mocks.enable.mockResolvedValue(undefined);mocks.refresh.mockResolvedValue([account("codex")]);});
afterEach(()=>{vi.useRealTimers();vi.restoreAllMocks();});
describe("Usage presentation",()=>{
  it("formats large counts without treating missing usage as zero",()=>{
    expect([null,0,1200,1234567,1e9].map(formatTokens)).toEqual(["—","0","1.2K","1.23M","1B"]);
    const {rerender}=render(<TokenBadge t={t}/>);expect(screen.getByTitle(t("usageMissing"))).toHaveTextContent("—");
    rerender(<TokenBreakdown t={t} usage={{totalTokens:1200,inputTokens:1000,outputTokens:200,cacheReadTokens:500,cacheWriteTokens:0,reasoningTokens:50,model:"model",partial:false}}/>);
    expect(screen.getByText("1.2K")).toBeInTheDocument();expect(screen.getByText(t("usageSubsetHint"))).toBeInTheDocument();
  });
  it("labels used and remaining distinctly, and never resets an expired observation to zero",()=>{
    const w=account("codex").windows[0];const now=Date.parse("2026-10-02T00:00:00Z");
    render(<QuotaMeter window={w} t={t} now={now}/>);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuenow","25");
    expect(screen.getByText(/remaining/).parentElement).toHaveTextContent("75% remaining");
    expect(screen.getByText(t("usageResetPending"))).toBeInTheDocument();
    expect(resetLabel(null,now,t)).toBe(t("usageResetUnknown"));
  });
  it("does not request disabled accounts and requires an explicit connection action",async()=>{
    render(<UsagePanel navigation={navigation} t={t} onClose={vi.fn()}/>);
    await screen.findByRole("button",{name:`${t("usageConnect")} · Codex`});expect(mocks.refresh).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button",{name:`${t("usageConnect")} · Codex`}));
    expect(mocks.enable).not.toHaveBeenCalled();
    const dialog=screen.getByRole("alertdialog");expect(dialog).toHaveTextContent("chatgpt.com");
    fireEvent.click(within(dialog).getByRole("button",{name:t("usageConnectAction")}));
    await waitFor(()=>expect(mocks.enable).toHaveBeenCalledWith("codex",true));
    await waitFor(()=>expect(mocks.refresh).toHaveBeenCalledWith("codex",false));
  });
  it("merges concurrent provider responses without overwriting a newer other-provider result",async()=>{
    const initial=[account("codex"),account("claude")];mocks.subscriptions.mockResolvedValue(initial);
    let resolveCodex!:(v:SubscriptionUsage[])=>void,resolveClaude!:(v:SubscriptionUsage[])=>void;
    mocks.refresh.mockImplementation((provider:string)=>new Promise(resolve=>{if(provider==="codex")resolveCodex=resolve;else resolveClaude=resolve;}));
    render(<UsagePanel navigation={navigation} t={t} onClose={vi.fn()}/>);await waitFor(()=>expect(mocks.refresh).toHaveBeenCalledTimes(2));
    const freshCodex={...initial[0],plan:"Pro"};const freshClaude={...initial[1],plan:"Max"};
    await act(async()=>resolveCodex([freshCodex,initial[1]]));
    await act(async()=>resolveClaude([initial[0],freshClaude]));
    expect(screen.getByText("Pro")).toBeInTheDocument();expect(screen.getByText("Max")).toBeInTheDocument();
  });
  it("shows token coverage and only cached metadata in the history view",async()=>{
    mocks.summary.mockResolvedValue([{adapter:"codex",sessions:5,measuredSessions:3,totalTokens:1200000}]);
    render(<UsagePanel navigation={navigation} t={t} onClose={vi.fn()}/>);
    await screen.findByRole("button",{name:t("usageHistory")});fireEvent.click(screen.getByRole("button",{name:t("usageHistory")}));
    expect(screen.getByText("Codex CLI")).toBeInTheDocument();expect(screen.getAllByText("3 / 5 sessions").length).toBeGreaterThan(0);
    expect(mocks.refresh).not.toHaveBeenCalled();
  });
  it("keeps the page open when focus moves to persistent title-bar controls",async()=>{
    const close=vi.fn();render(<><button>Window controls</button><UsagePanel navigation={navigation} t={t} onClose={close}/></>);
    await screen.findByRole("button",{name:`${t("usageConnect")} · Codex`});
    await act(async()=>{screen.getByRole("button",{name:"Window controls"}).focus();});
    expect(close).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button",{name:t("close")}));expect(close).toHaveBeenCalledOnce();
  });
  it("marks partially priced token counts and leaves unknown costs absent",()=>{
    const usage:TokenUsage={totalTokens:2000,inputTokens:1800,outputTokens:200,cacheReadTokens:0,cacheWriteTokens:0,reasoningTokens:null,model:"mixed",partial:false,cost:{usd:.004,pricedTokens:1000,unpricedTokens:1000,priceDate:"2026-09-30",assumptions:false}};
    const {rerender}=render(<TokenBreakdown t={t} usage={usage}/>);
    expect(screen.getByText(/≈ <\$0.01/,{selector:".token-cost"})).toHaveTextContent(t("usagePricePartialShort"));
    expect(screen.getByText(/Pricing coverage 50%/)).toBeInTheDocument();
    rerender(<TokenBadge t={t} usage={{...usage,cost:{...usage.cost!,pricedTokens:0,unpricedTokens:2000,usd:0}}}/>);
    expect(document.querySelector(".token-cost")).toBeNull();
    expect(formatMoney(null)).toBe("—");expect(formatMoney(0)).toBe("$0.00");
  });
  it("updates an open usage page from a title-bar refresh without refetching transcripts",async()=>{
    mocks.subscriptions.mockResolvedValue([account("codex")]);
    render(<UsagePanel navigation={navigation} t={t} onClose={vi.fn()}/>);
    await waitFor(()=>expect(mocks.refresh).toHaveBeenCalledOnce());
    await act(async()=>notifyUsageChanged({...account("codex"),plan:"Updated plan"}));
    expect(screen.getByText("Updated plan")).toBeInTheDocument();
  });
});

describe("Persistent title-bar quotas",()=>{
  it("hides selected providers immediately without disconnecting accounts or losing the full popover",async()=>{
    mocks.subscriptions.mockResolvedValue([account("codex"),account("claude"),account("kimi")]);
    const manage=vi.fn();
    const view=render(<TitleBarUsage navigation={{...navigation, hidden:["codex"]}} t={t} onOpen={manage} active={false}/>);
    const trigger=await screen.findByRole("button",{name:/Subscription limits.*Claude/});
    expect(trigger).not.toHaveTextContent("Codex");expect(trigger).toHaveTextContent("Kimi");
    fireEvent.click(trigger);expect(await screen.findByText("Codex")).toBeInTheDocument();
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    expect(screen.queryByText(t("usageNavSettingsScope"))).not.toBeInTheDocument();
    view.rerender(<TitleBarUsage navigation={{...navigation, hidden:["codex","claude","kimi"]}} t={t} onOpen={manage} active={false}/>);
    expect(screen.getByRole("button",{name:/Subscription limits/})).toHaveTextContent(t("usageTitle"));
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:t("usageManage")}));expect(manage).toHaveBeenCalledOnce();
    expect(mocks.enable).not.toHaveBeenCalled();
  });
  it("shows every visible provider inline and exposes depleted windows, resets and management",async()=>{
    const go=account("opencode-go");go.windows=[{...go.windows[0],id:"short",label:"rolling",windowMinutes:300,usedPercent:0},{...go.windows[0],id:"monthly",label:"monthly",windowMinutes:43200,usedPercent:100}];
    mocks.subscriptions.mockResolvedValue([account("codex"),go,account("claude"),account("grok"),account("antigravity")]);
    const manage=vi.fn();render(<TitleBarUsage navigation={navigation} t={t} onOpen={manage} active={false}/>);
    const trigger=await screen.findByRole("button",{name:/Codex 75% remaining.*OpenCode Go 0% remaining/});
    for(const name of ["Codex","Go","Claude","Grok","Google Antigravity"]) expect(trigger).toHaveTextContent(name);
    expect(trigger.querySelectorAll(".nav-quota")).toHaveLength(5);
    expect(trigger.querySelector(".nav-quota-more")).toBeNull();expect(trigger).toHaveTextContent("0%");
    fireEvent.click(trigger);
    expect(await screen.findByText("100%")).toBeInTheDocument();
    expect(screen.getAllByText(/Reset/).length).toBeGreaterThan(0);
    expect(screen.getByRole("meter",{name:/OpenCode Go.*30 d/})).toHaveAttribute("aria-valuenow","100");
    fireEvent.click(screen.getByRole("button",{name:t("usageManage")}));expect(manage).toHaveBeenCalledOnce();
  });
  it("scrolls crowded quotas by wheel and keyboard without changing connections",async()=>{
    mocks.subscriptions.mockResolvedValue([account("codex"),account("claude"),account("grok")]);
    render(<TitleBarUsage navigation={navigation} t={t} onOpen={vi.fn()} active={false}/>);
    const trigger=await screen.findByRole("button",{name:/Subscription limits.*Grok/});
    const rail=trigger.parentElement!;
    Object.defineProperties(rail,{scrollWidth:{value:600,configurable:true},clientWidth:{value:250,configurable:true}});
    fireEvent.wheel(rail,{deltaY:80});expect(rail.scrollLeft).toBe(80);
    fireEvent.keyDown(trigger,{key:"ArrowRight"});expect(rail.scrollLeft).toBe(180);
    fireEvent.keyDown(trigger,{key:"ArrowLeft"});expect(rail.scrollLeft).toBe(80);
    fireEvent.wheel(rail,{deltaY:80,ctrlKey:true});expect(rail.scrollLeft).toBe(80);
    Object.defineProperty(rail,"clientWidth",{value:600});
    fireEvent.wheel(rail,{deltaY:80});expect(rail.scrollLeft).toBe(80);
    expect(mocks.enable).not.toHaveBeenCalled();expect(mocks.refresh).not.toHaveBeenCalled();
  });
  it("loads cache immediately and bounds network refresh to three visible enabled connections",async()=>{
    vi.useFakeTimers();vi.setSystemTime(new Date("2026-09-30T08:00:00Z"));
    mocks.subscriptions.mockResolvedValue([account("codex"),account("claude"),account("copilot"),account("kimi"),account("deepseek",false)]);
    const finish:Array<()=>void>=[];
    mocks.refresh.mockImplementation((provider:string)=>new Promise(resolve=>finish.push(()=>resolve([account(provider)]))));
    const view=render(<TitleBarUsage navigation={navigation} t={t} onOpen={vi.fn()} active={false}/>);
    await act(async()=>{});expect(mocks.subscriptions).toHaveBeenCalledOnce();expect(mocks.refresh).not.toHaveBeenCalled();
    await act(async()=>vi.advanceTimersByTimeAsync(1500));expect(mocks.refresh).toHaveBeenCalledTimes(3);
    const hidden=vi.spyOn(document,"hidden","get").mockReturnValue(true);
    await act(async()=>{finish.forEach(resolve=>resolve());});
    await act(async()=>vi.advanceTimersByTimeAsync(300000));expect(mocks.refresh).toHaveBeenCalledTimes(3);
    hidden.mockReturnValue(false);
    view.unmount();
    expect(mocks.refresh.mock.calls.every(([provider])=>provider!=="deepseek")).toBe(true);
  });
  it("keeps disabled providers offline and distinguishes weekly quotas from short windows",async()=>{
    vi.useFakeTimers();
    const view=render(<TitleBarUsage navigation={navigation} t={t} onOpen={vi.fn()} active={false}/>);
    await act(async()=>vi.advanceTimersByTimeAsync(301500));expect(mocks.refresh).not.toHaveBeenCalled();
    expect(windowLabel({...account("codex").windows[0],label:"rolling"},t)).toBe(t("usageWeekly"));
    expect(staleUsage({...account("codex"),fetchedAt:"invalid"},Date.now())).toBe(true);
    view.unmount();
  });
});

function NavigationHarness({hidden=[], onChange}:{hidden?:string[];onChange:(hidden:string[])=>Promise<void>}) {
  const current=useUsageNavigation(hidden,onChange);
  return <><UsageNavSwitch provider="codex" navigation={current} t={t}/><UsageNavSwitch provider="claude" navigation={current} t={t}/><UsageNavFeedback navigation={current} t={t}/></>;
}
describe("Navigation visibility settings",()=>{
  it("blocks overlapping saves, shows failure, and retains the previous value until persistence succeeds",async()=>{
    let reject!:(reason:Error)=>void;
    const change=vi.fn().mockImplementationOnce(()=>new Promise((_resolve,no)=>{reject=no;})).mockResolvedValue(undefined);
    const view=render(<NavigationHarness hidden={[]} onChange={change}/>);
    const codex=screen.getByRole("switch",{name:`${t("usageNavShow")} · Codex`});
    fireEvent.click(codex);expect(change).toHaveBeenCalledWith(["codex"]);
    expect(codex).toHaveAttribute("aria-disabled","true");expect(screen.getByRole("switch",{name:`${t("usageNavShow")} · Claude`})).toHaveAttribute("aria-disabled","true");
    expect(screen.getByRole("status")).toHaveTextContent(t("usageNavSaving"));
    await act(async()=>reject(new Error("disk full")));
    expect(codex).toHaveAttribute("aria-checked","true");
    expect(screen.getByRole("alert")).toHaveTextContent(t("settingsSaveFailed"));
    await act(async()=>fireEvent.click(codex));
    view.rerender(<NavigationHarness hidden={["codex"]} onChange={change}/>);
    expect(codex).toHaveAttribute("aria-checked","false");
    expect(screen.getByRole("status")).toHaveTextContent(t("settingsSaved"));
    await act(async()=>fireEvent.click(codex));
    expect(change).toHaveBeenLastCalledWith([]);
    expect(mocks.enable).not.toHaveBeenCalled();
  });
});
