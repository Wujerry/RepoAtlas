export type ResourceSnapshot<T> = { value?: T; loading: boolean; error?: string; updatedAt: number };
export class InvalidatedResource extends Error {}

/** Bounded, memory-only LRU with single-flight reads and invalidation fencing. */
export class ResourceCache<T> {
  private entries = new Map<string, ResourceSnapshot<T>>();
  private pending = new Map<string, Promise<T>>();
  private listeners = new Set<() => void>();
  revision = 0;

  constructor(private loader: (key: string) => Promise<T>, private ttl: number, private limit = 20) {}
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  peek = (key: string) => this.entries.get(key);
  private emit() { this.listeners.forEach((listener) => listener()); }
  private put(key: string, value: ResourceSnapshot<T>) {
    this.entries.delete(key);
    this.entries.set(key, value);
    while (this.entries.size > this.limit) this.entries.delete(this.entries.keys().next().value!);
    this.emit();
  }
  clear() {
    this.revision += 1;
    this.entries.clear();
    this.pending.clear();
    this.emit();
  }
  read(key: string, force = false): Promise<T> {
    const pending = this.pending.get(key);
    if (pending) return pending;
    const cached = this.entries.get(key);
    if (!force && cached?.value !== undefined && Date.now() - cached.updatedAt < this.ttl) {
      this.entries.delete(key);
      this.entries.set(key, cached);
      return Promise.resolve(cached.value);
    }
    const revision = this.revision;
    const request = Promise.resolve().then(() => {
      if (revision !== this.revision) throw new InvalidatedResource();
      return this.loader(key);
    }).then((value) => {
      if (revision !== this.revision) throw new InvalidatedResource();
      if (this.entries.has(key)) {
        this.entries.set(key, { value, loading: false, updatedAt: Date.now() });
        this.emit();
      }
      return value;
    }).catch((error: unknown) => {
      if (revision !== this.revision) throw new InvalidatedResource();
      if (this.entries.has(key)) {
        this.entries.set(key, { value: cached?.value, loading: false, error: String(error), updatedAt: cached?.updatedAt ?? 0 });
        this.emit();
      }
      throw error;
    }).finally(() => { if (this.pending.get(key) === request) this.pending.delete(key); });
    this.pending.set(key, request);
    this.put(key, { value: cached?.value, loading: true, updatedAt: cached?.updatedAt ?? 0 });
    return request;
  }
}
