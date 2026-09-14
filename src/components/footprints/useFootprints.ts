import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../lib/api";
import { calendarDays, dayRange, localDate, offsetDate } from "../../lib/footprints";
import type { ActivityHistoryItem, ActivityHistoryResponse, ActivitySummary, HistoryRefreshStatus } from "../../types";

type CachePage = { items: ActivityHistoryItem[]; nextCursor?: string | null; pages: number };
export function useFootprints(open: boolean) {
  const [date, setDate] = useState(localDate);
  const [calendarEnd, setCalendarEnd] = useState(localDate);
  const [project, setProject] = useState("");
  const [category, setCategory] = useState("");
  const [search, setSearch] = useState("");
  const [debounced, setDebounced] = useState("");
  const [page, setPage] = useState<CachePage>({ items: [], pages: 0 });
  const [summary, setSummary] = useState<ActivitySummary>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [summaryError, setSummaryError] = useState("");
  const [selectedId, setSelectedId] = useState<string>();
  const [detail, setDetail] = useState<ActivityHistoryItem>();
  const [detailError, setDetailError] = useState("");
  const [refresh, setRefresh] = useState<HistoryRefreshStatus>();
  const [refreshError, setRefreshError] = useState("");
  const [starting, setStarting] = useState(false);
  const [hasUpdates, setHasUpdates] = useState(false);
  const [revision, setRevision] = useState(0);
  const cache = useRef(new Map<string, CachePage>());
  const serial = useRef(0);
  const pending = useRef(false);
  const automaticJob = useRef<string | undefined>(undefined);
  const alive = useRef(open);
  const openEpoch = useRef(0);
  const initialLoaded = useRef(false);
  const preparedKey = useRef<string | undefined>(undefined);
  const landing = useRef<"start" | "end">("start");
  const days = useMemo(() => calendarDays(calendarEnd), [calendarEnd]);
  const query = useMemo(() => ({ ...dayRange(date), projectId: project || undefined, category: category || undefined, search: debounced || undefined, limit: 50 }), [date, project, category, debounced]);
  const key = JSON.stringify(query);
  const [pageKey, setPageKey] = useState(key);
  useEffect(() => { const timer = setTimeout(() => setDebounced(search.trim()), 180); return () => clearTimeout(timer); }, [search]);
  const saveCache = useCallback((k: string, value: CachePage) => {
    cache.current.delete(k); cache.current.set(k, value);
    while (cache.current.size > 8) cache.current.delete(cache.current.keys().next().value!);
  }, []);

  useEffect(() => {
    alive.current = open; openEpoch.current++;
    if (!open) return;
    initialLoaded.current = false;
    return () => {
      alive.current = false; openEpoch.current++; serial.current++;
      if (automaticJob.current) void api.cancelGitHistoryRefresh(automaticJob.current).catch(() => {});
      automaticJob.current = undefined;
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const request = ++serial.current; pending.current = false; setError(""); setPageKey(key);
    const cached = cache.current.get(key);
    if (preparedKey.current === key && cached) {
      preparedKey.current = undefined; setPage(cached); setLoading(false); initialLoaded.current = true;
      return;
    }
    landing.current = "start";
    if (cached) { setPage(cached); setLoading(false); initialLoaded.current = true; }
    else { setPage({ items: [], pages: 0 }); setLoading(true); }
    pending.current = true;
    api.getActivityHistory(query).then((result: ActivityHistoryResponse) => {
      if (request !== serial.current || !alive.current) return;
      const value = { items: result.items, nextCursor: result.nextCursor, pages: 1 };
      setPage(value); saveCache(key, value); initialLoaded.current = true;
    }).catch(reason => { if (request === serial.current) setError(String(reason)); })
      .finally(() => { if (request === serial.current) { setLoading(false); pending.current = false; } });
    return () => { if (request === serial.current) serial.current++; };
  }, [open, key, revision, query, saveCache]);

  useEffect(() => {
    if (!open) return;
    let canceled = false; setSummaryError("");
    api.getActivitySummary(days, project || undefined).then(result => { if (!canceled) setSummary(result); })
      .catch(reason => { if (!canceled) setSummaryError(String(reason)); });
    return () => { canceled = true; };
  }, [open, days, project, revision]);

  useEffect(() => {
    if (!open) return;
    if (page.items.length && !page.items.some(item => item.id === selectedId)) setSelectedId(page.items[0].id);
    if (!loading && !page.items.length) setSelectedId(undefined);
  }, [page.items, loading, selectedId, open]);

  useEffect(() => {
    let canceled = false; setDetail(undefined); setDetailError("");
    if (open && selectedId) api.getActivityDetail(selectedId).then(result => { if (!canceled) setDetail(result); })
      .catch(reason => { if (!canceled) setDetailError(String(reason)); });
    return () => { canceled = true; };
  }, [open, selectedId, revision]);

  const loadMore = useCallback(async () => {
    if (pending.current || !page.nextCursor || page.pages >= 10) return;
    const request = serial.current; pending.current = true; setLoading(true); setError("");
    try {
      const result = await api.getActivityHistory({ ...query, cursor: page.nextCursor });
      if (request !== serial.current || !alive.current) return;
      const items = [...new Map([...page.items, ...result.items].map(item => [item.id, item])).values()];
      const value = { items, nextCursor: result.nextCursor, pages: page.pages + 1 };
      setPage(value); saveCache(key, value);
    } catch (reason) { if (request === serial.current) setError(String(reason)); }
    finally { if (request === serial.current) { pending.current = false; setLoading(false); } }
  }, [page, query, key, saveCache]);

  // Continue beyond the memory window without retaining an unbounded list.
  const nextWindow = useCallback(async () => {
    if (!page.nextCursor || pending.current) return;
    const request = serial.current; pending.current = true; setLoading(true);
    try {
      const result = await api.getActivityHistory({ ...query, cursor: page.nextCursor });
      if (request !== serial.current || !alive.current) return;
      const value = { items: result.items, nextCursor: result.nextCursor, pages: 1 };
      setPage(value); saveCache(key, value);
    } catch (reason) { if (request === serial.current) setError(String(reason)); }
    finally { if (request === serial.current) { pending.current = false; setLoading(false); } }
  }, [page.nextCursor, query, key, saveCache]);

  // Prepare the adjacent day without clearing the day the user is still reading.
  const loadAdjacentDay = useCallback(async (direction: -1 | 1) => {
    if (pending.current || pageKey !== key || (direction === -1 && page.nextCursor)) return;
    const target = offsetDate(date, direction);
    if (target > localDate()) return;
    const request = serial.current;
    pending.current = true; setLoading(true); setError("");
    const targetQuery = { ...query, ...dayRange(target) };
    const targetKey = JSON.stringify(targetQuery);
    try {
      const result = await api.getActivityHistory(targetQuery);
      const items = result.items;
      if (request !== serial.current || !alive.current) return;
      const value = { items: [...new Map(items.map(item => [item.id, item])).values()], nextCursor: result.nextCursor, pages: Math.max(1, Math.ceil(items.length / 50)) };
      saveCache(targetKey, value); preparedKey.current = targetKey;
      landing.current = direction === 1 && !result.nextCursor ? "end" : "start";
      setPage(value); setPageKey(targetKey); setDate(target);
      if (target > calendarEnd || target < offsetDate(calendarEnd, -29)) setCalendarEnd(target);
    } catch (reason) { if (request === serial.current) setError(String(reason)); }
    finally { if (request === serial.current) { pending.current = false; setLoading(false); } }
  }, [date, calendarEnd, query, key, pageKey, page.nextCursor, saveCache]);

  const startRefresh = useCallback(async (automatic = false, historical = false) => {
    setStarting(true); setRefreshError("");
    const today = localDate();
    const epoch = openEpoch.current;
    try {
      if (automatic) {
        const existing = await api.getGitHistoryRefreshStatus();
        if (!alive.current || epoch !== openEpoch.current) return;
        if (existing.state === "running") { setRefresh(existing); return; }
      }
      const status = await api.startGitHistoryRefresh({ projectId: historical ? project || undefined : undefined, startAt: dayRange(historical ? days[0].date : offsetDate(today, -89)).startAt, endAt: dayRange(historical ? days[29].date : today).endAt, force: !automatic });
      if (!alive.current || epoch !== openEpoch.current) { if (automatic) void api.cancelGitHistoryRefresh(status.id).catch(() => {}); return; }
      if (automatic) automaticJob.current = status.id;
      setRefresh(status);
    } catch (reason) { if (alive.current && epoch === openEpoch.current) setRefreshError(String(reason)); }
    finally { if (alive.current && epoch === openEpoch.current) setStarting(false); }
  }, [project, days]);
  useEffect(() => {
    if (!open) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const begin = () => {
      if (stopped) return;
      if (!document.hidden && initialLoaded.current) { void startRefresh(true); return; }
      timer = setTimeout(begin, 100);
    };
    begin();
    return () => { stopped = true; clearTimeout(timer); };
  }, [open]);
  useEffect(() => {
    if (!open || refresh?.state !== "running") return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (stopped) return;
      if (!document.hidden) {
        try {
          const status = await api.getGitHistoryRefreshStatus();
          if (stopped) return;
          if (status.id === refresh.id) {
            setRefresh(status);
            if (status.state !== "running") { if (status.commits > 0) setHasUpdates(true); return; }
          }
        } catch (reason) { if (!stopped) setRefreshError(String(reason)); }
      }
      if (!stopped) timer = setTimeout(poll, 500);
    };
    timer = setTimeout(poll, 500);
    return () => { stopped = true; clearTimeout(timer); };
  }, [open, refresh?.id, refresh?.state]);

  const reload = useCallback(() => { cache.current.clear(); setHasUpdates(false); setRevision(n => n + 1); }, []);
  const selectDate = useCallback((value: string) => {
    setDate(value); if (value > calendarEnd || value < offsetDate(calendarEnd, -29)) setCalendarEnd(value);
  }, [calendarEnd]);
  return { date, selectDate, calendarEnd, setCalendarEnd, project, setProject, category, setCategory, search, setSearch,
    page: pageKey === key ? page : { items: [], pages: 0 }, summary, loading: loading || pageKey !== key, error, summaryError, selectedId, setSelectedId, detail, detailError, refresh, refreshError,
    starting, hasUpdates, days, key, landing, loadAdjacentDay, loadMore, nextWindow, startRefresh, reload,
    cancelRefresh: async () => { if (refresh) { try { await api.cancelGitHistoryRefresh(refresh.id); } catch (reason) { setRefreshError(String(reason)); } } },
  };
}
