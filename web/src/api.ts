/** Cosa deve fare l'utente prima di poter usare il pannello. */
export type Requirement = "change_password" | "setup_2fa" | null;

export interface SessionUser {
  username: string;
  role: string;
  scopes: string[];
  two_factor: boolean;
  requirement: Requirement;
}

export type TwoFactorPolicy = "off" | "all" | "managers";

export interface Session {
  setup_required: boolean;
  authenticated: boolean;
  username: string | null;
  user: SessionUser | null;
  policy: { require_2fa: TwoFactorPolicy };
  /** Dove sta la guida (copia offline o online); assente = nessun link */
  docs_url?: string | null;
}

export interface UserInfo {
  id: string;
  username: string;
  role: string;
  scopes: string[];
  custom_scopes: string[];
  disabled: boolean;
  two_factor: boolean;
  must_change_password: boolean;
  created_at: string;
  last_login_at: string | null;
}

export interface ScopeInfo {
  id: string;
  description: string;
}

export interface AuditEntry {
  ts: string;
  user: string;
  action: string;
  target: string;
  ok: boolean;
}

export type LoginResult = { ok: true } | { needs_2fa: true; challenge: string };

export interface TwoFactorStart {
  secret: string;
  otpauth_url: string;
  qr_svg: string | null;
}

export interface Stage {
  id: string;
  label: string;
  status: "ok" | "fail" | "skip";
  message: string;
}

export interface CheckResult {
  ok: boolean;
  records: string[];
  message: string;
  stages: Stage[];
}

/** Stato mostrato: in attesa (mai verificato), verificato o in errore (era valido e non lo è più). */
export type DomainStatus = "pending" | "verified" | "dns_error" | "unreachable";

export function domainStatus(d: { verified: boolean; ever_verified: boolean; stages: { id: string; status: string }[] }): DomainStatus {
  if (d.verified) return "verified";
  if (!d.ever_verified) return "pending";
  const reachOnly = d.stages.some((s) => s.id === "reach" && s.status === "fail") && d.stages.every((s) => s.id === "reach" || s.status !== "fail");
  return reachOnly ? "unreachable" : "dns_error";
}

export interface DomainInfo {
  host: string;
  verified: boolean;
  checked_at: string | null;
  records: string[];
  message: string;
  stages: Stage[];
  ever_verified: boolean;
  since: string | null;
  redirect_https: boolean;
}

export interface UpdateRelease {
  version: string;
  notes: string;
  url: string;
  published_at: string;
  prerelease: boolean;
}

export interface UpdateInfo {
  current: string;
  kind: "docker" | "service" | "binary" | "source";
  enabled: boolean;
  env_disabled: boolean;
  available: boolean;
  latest: UpdateRelease | null;
  checked_at: number;
  error: string | null;
  /** versione da cui si è appena aggiornato (24 ore) */
  updated_from: string | null;
  rollback: { to: string; reason: string; at: number } | null;
  can_self_update: boolean;
  self_update_blocked: string | null;
  apply: { running: boolean; step: string; error: string | null; restarting: boolean };
}

export interface CertInfo {
  host: string;
  status: "valid" | "expiring" | "expired" | "missing" | "error" | "issuing";
  not_after: number | null;
  issued_at: number | null;
  error: { at: number; message: string } | null;
  /** il nodo lo sta servendo su HTTPS */
  serving: boolean;
}

export interface AcmeSettings {
  enabled: boolean;
  email: string;
  staging: boolean;
}

export interface BucketCheck {
  outcome: "found" | "not_found" | "auth" | "unreachable" | "invalid";
  ok: boolean;
  message: string;
  checked_at: string;
}

export interface BucketInfo {
  id: string;
  name: string;
  endpoint: string;
  region: string;
  addressing: "path" | "virtual";
  allow_private_endpoint: boolean;
  bucket: string;
  test_file: string;
  check: BucketCheck | null;
}

export interface WarmResult {
  path: string;
  status: number | null;
  x_cache?: string | null;
  bytes?: number;
  elapsed_ms?: number;
  error: string | null;
}

export interface RuleInfo {
  id: string;
  domain: string;
  path_prefix: string;
  bucket_id: string;
  folder: string;
  cache_generation: number;
  signed: boolean;
  images: boolean;
}

export interface LiveRoute {
  id: string;
  match: string;
  bucket: string;
  prefix: string;
  storage: string;
}

export interface PanelState {
  version: string;
  config_version: number;
  listen_port: number;
  http_port: number;
  https_port: number;
  cache: { bytes: number; entries: number; max_bytes: number; inflight: number };
  live_routes: LiveRoute[];
  hand_managed: boolean;
  /** canali di notifica attivi il cui ultimo invio è fallito (solo con notifications:manage) */
  notify_failing?: string[];
  https_listening: boolean;
  update: UpdateInfo;
  certs: CertInfo[];
  recheck_minutes: number;
  recheck_verified_minutes: number;
  panel: {
    settings: { http_port: number | null; https_port: number | null; acme: AcmeSettings; admin_host: string | null; admin_allow: string[]; updates: { check: boolean; prerelease: boolean; auto: boolean; window_start: number; window_end: number } };
    domains: DomainInfo[];
    buckets: BucketInfo[];
    rules: RuleInfo[];
  };
}

export type MetricsRange = "1h" | "24h" | "7d";

export interface MetricsResponse {
  range: MetricsRange;
  totals: {
    requests: number;
    hit_ratio: number | null;
    bytes: number;
    errors_5xx: number;
    upstream_errors: number;
    latency: { p50: number | null; p95: number | null; p99: number | null };
  };
  classes: { "2xx": number; "3xx": number; "4xx": number; "5xx": number };
  latency_histogram: { le: number | null; count: number }[];
  series: { t: number; req: number; hit: number; miss: number; other: number; bytes: number; err: number }[];
  routes: {
    id: string;
    match: string | null;
    requests: number;
    hit_ratio: number | null;
    bytes: number;
    errors: number;
    upstream_errors: number;
    p95: number | null;
  }[];
  top_files: { route: string; match: string | null; path: string; requests: number; bytes: number }[];
  cache: { entries: number; bytes: number; max_bytes: number; inflight: number };
  config_version: number;
}

export interface StorageInput {
  endpoint: string;
  region: string;
  addressing: "path" | "virtual";
  allow_private_endpoint: boolean;
  access_key: string;
  secret_key: string;
}

export interface TestResult {
  outcome: "found" | "not_found" | "auth" | "unreachable" | "invalid";
  ok: boolean;
  message: string;
  size?: string;
  content_type?: string;
}

export interface ProbeResult {
  status: number;
  x_cache: string | null;
  content_type: string | null;
  content_length: string | null;
  elapsed_ms: number;
  preview: string | null;
}

export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message);
  }
}

async function request<T>(path: string, method = "GET", body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new ApiError(data.error ?? `Errore ${res.status}`, res.status);
  return data as T;
}

const post = <T>(path: string, body: unknown = {}) => request<T>(path, "POST", body);
const del = (path: string) => request<{ ok: boolean }>(path, "DELETE");

export type Threshold = "warnings" | "errors";

export interface NotifyConfig {
  delay_min: number;
  recovery: boolean;
  reminder_hours: number;
  email: {
    enabled: boolean;
    threshold: Threshold;
    host: string;
    port: number;
    security: "tls" | "starttls" | "none";
    user: string;
    from: string;
    to: string[];
  };
  telegram: { enabled: boolean; threshold: Threshold; chat_ids: string[] };
}

export interface NotifyLogEntry {
  at: string;
  channel: "email" | "telegram";
  kind: "problem" | "recovered" | "reminder" | "test";
  title: string;
  ok: boolean;
  error: string | null;
}

export interface NotifyInfo {
  config: NotifyConfig;
  has_smtp_password: boolean;
  has_telegram_token: boolean;
  log: NotifyLogEntry[];
}

export interface DiagStep {
  id: string;
  label: string;
  status: "ok" | "warn" | "fail" | "skip";
  detail: string;
  fix?: string;
}

export interface DiagResult {
  steps: DiagStep[];
  summary: string;
  report: string;
}

export const api = {
  diagnose: (url: string) => post<DiagResult>("/api/diagnose", { url }),
  notifications: () => request<NotifyInfo>("/api/notifications"),
  saveNotifications: (b: { config: NotifyConfig; smtp_password?: string; telegram_token?: string }) =>
    request<NotifyInfo>("/api/notifications", "PUT", b),
  testNotification: (channel: "email" | "telegram") =>
    post<{ ok: boolean; error?: string; log: NotifyLogEntry[] }>("/api/notifications/test", { channel }),
  users: () => request<{ users: UserInfo[]; scopes: ScopeInfo[]; policy: { require_2fa: TwoFactorPolicy } }>("/api/users"),
  createUser: (b: { username: string; password?: string; role: string; scopes?: string[] }) =>
    post<{ user: UserInfo; password: string }>("/api/users", b),
  updateUser: (id: string, b: { role?: string; scopes?: string[]; disabled?: boolean }) =>
    request<UserInfo>(`/api/users/${encodeURIComponent(id)}`, "PUT", b),
  deleteUser: (id: string) => del(`/api/users/${encodeURIComponent(id)}`),
  resetPassword: (id: string) => post<{ password: string }>(`/api/users/${encodeURIComponent(id)}/reset-password`),
  resetTwoFactor: (id: string) => post<UserInfo>(`/api/users/${encodeURIComponent(id)}/reset-2fa`),
  setPolicy: (require_2fa: TwoFactorPolicy) => request<{ require_2fa: TwoFactorPolicy }>("/api/policy", "PUT", { require_2fa }),
  audit: (limit = 100, before?: string) =>
    request<{ entries: AuditEntry[] }>(`/api/audit?limit=${limit}${before ? `&before=${encodeURIComponent(before)}` : ""}`),

  me: () => request<{ user: UserInfo; policy: { require_2fa: TwoFactorPolicy } }>("/api/me"),
  changePassword: (current: string, next: string) => request<{ ok: boolean }>("/api/me/password", "PUT", { current, new: next }),
  twoFactorStart: () => post<TwoFactorStart>("/api/me/2fa/start"),
  twoFactorConfirm: (code: string) => post<{ recovery_codes: string[] }>("/api/me/2fa/confirm", { code }),
  twoFactorDisable: (password: string, body: { code?: string; recovery_code?: string }) =>
    post<{ ok: boolean }>("/api/me/2fa/disable", { password, ...body }),
  twoFactorRecovery: (password: string) => post<{ recovery_codes: string[] }>("/api/me/2fa/recovery", { password }),

  session: () => request<Session>("/api/session"),
  setup: (username: string, password: string) => post<{ username: string }>("/api/setup", { username, password }),
  login: (username: string, password: string) => post<LoginResult>("/api/login", { username, password }),
  login2fa: (challenge: string, body: { code?: string; recovery_code?: string }) =>
    post<{ ok: true }>("/api/login/2fa", { challenge, ...body }),
  logout: () => post<{ ok: boolean }>("/api/logout"),

  panel: () => request<PanelState>("/api/panel"),
  metrics: (range: MetricsRange) => request<MetricsResponse>(`/api/metrics?range=${range}`),
  saveSettings: (http_port: number | null, https_port: number | null) =>
    request<{ http_port: number; https_port: number }>("/api/settings", "PUT", { http_port, https_port }),

  addDomain: (host: string) => post<DomainInfo>("/api/domains", { host }),
  checkDomain: (host: string) => post<DomainInfo>("/api/domains/check", { host }),
  testDomain: (host: string) => post<{ host: string; result: CheckResult }>("/api/domains/test", { host }),
  deleteDomain: (host: string) => del(`/api/domains/${encodeURIComponent(host)}`),

  testBucket: (b: StorageInput & { bucket: string; prefix: string; file: string }) =>
    post<TestResult>("/api/buckets/test", b),
  addBucket: (b: StorageInput & { name: string; bucket: string; file: string }) =>
    post<BucketInfo>("/api/buckets", b),
  checkBucket: (id: string) => post<BucketCheck>("/api/buckets/check", { id }),
  deleteBucket: (id: string) => del(`/api/buckets/${encodeURIComponent(id)}`),

  addRule: (r: { domain: string; path_prefix: string; bucket_id: string; folder: string }) =>
    post<RuleInfo>("/api/rules", r),
  setAdminAllow: (list: string[]) => request<{ list: string[] }>("/api/admin-allow", "PUT", { list }),
  setAdminHost: (host: string | null) =>
    request<{ host: string | null; url: string | null; warnings: string[] }>("/api/admin-host", "PUT", { host }),
  saveUpdates: (b: { check: boolean; prerelease: boolean; auto?: boolean; window_start?: number; window_end?: number }) => request<UpdateInfo>("/api/updates", "PUT", b),
  applyUpdate: () => post<{ started: boolean }>("/api/update/apply"),
  checkUpdate: () => post<UpdateInfo>("/api/update/check"),
  saveHttps: (b: AcmeSettings) => request<AcmeSettings>("/api/https", "PUT", b),
  issueCert: (host: string) => post<{ started: boolean }>("/api/certs/issue", { host }),
  uploadCert: (host: string, chain: string, key: string) =>
    post<{ host: string; not_after: number }>("/api/certs/upload", { host, chain, key }),
  setRedirect: (host: string, enabled: boolean) =>
    post<{ host: string; redirect_https: boolean }>("/api/domains/redirect", { host, enabled }),
  setRuleOptions: (id: string, o: { signed?: boolean; images?: boolean }) =>
    request<{ id: string; signed: boolean; images: boolean }>(`/api/rules/${encodeURIComponent(id)}`, "PUT", o),
  createLink: (b: { rule: string; path: string; ttl_secs: number; https?: boolean }) =>
    post<{ url: string; expires_at: number }>("/api/links", b),
  rotateLinks: () => post<{ ok: boolean }>("/api/links/rotate"),
  purgeCache: (rule: string, path?: string) => post<{ all?: boolean; removed?: number }>("/api/purge", { rule, path }),
  warmCache: (rule: string, paths: string[]) => post<{ results: WarmResult[] }>("/api/warm", { rule, paths }),
  deleteRule: (id: string) => del(`/api/rules/${encodeURIComponent(id)}`),

  probe: (host: string, path: string) => post<ProbeResult>("/api/probe", { host, path }),
};
