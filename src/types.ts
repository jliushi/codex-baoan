export type SourceStatus = "ok" | "missing" | "error";
export type ProviderStatus = "ready" | "needs-auth" | "auth-unverified" | "unconfigured";
export type GuardMode = "audit" | "block";
export type Severity = "info" | "low" | "medium" | "high" | "critical";
export type KnownProviderSource = "ccswitch" | "codexplusplus" | "codex-config";
export type ProviderSource = KnownProviderSource | (string & {});
export type ActivityKind = "command" | "file-read" | "file-create" | "file-delete" | "file-modify" | "network" | "risk";
export type ActivityFilter = "all" | ActivityKind;

export interface DiscoverySourceReport {
  id: string;
  label: string;
  path: string;
  exists: boolean;
  status: SourceStatus;
  provider_count: number;
  message: string;
}

export interface DiscoveredProvider {
  id: string;
  source: ProviderSource;
  source_label: string;
  source_path: string;
  native_id: string;
  name: string;
  base_url?: string;
  masked_api_key?: string;
  has_api_key: boolean;
  status: ProviderStatus;
  status_text: string;
  is_current: boolean;
  is_recommended: boolean;
  model?: string;
  protocol?: string;
  notes: string[];
}

export interface DiscoveryResult {
  generated_at: string;
  providers: DiscoveredProvider[];
  sources: DiscoverySourceReport[];
  recommended_provider_id?: string;
  manual_fallback_reason: string;
}

export interface RuntimeState {
  running: boolean;
  provider_id?: string;
  provider_name?: string;
  mode: GuardMode;
  started_at?: string;
  local_proxy_url?: string;
}

export interface ActivityEvent {
  id: string;
  timestamp: string;
  kind: ActivityKind;
  title: string;
  command?: string;
  paths: string[];
  severity: Severity;
  summary: string;
  line_delta?: number;
  lines_added?: number;
  lines_removed?: number;
  source?: string;
}

export interface AppInfo {
  version: string;
  install_dir: string;
  sessions_dir: string;
  bundle_managed: boolean;
  updater_configured: boolean;
  portable_mode: boolean;
}

export interface AppState {
  app: AppInfo;
  discovery: DiscoveryResult;
  runtime: RuntimeState;
  activity: ActivityEvent[];
}

export interface InspectDecision {
  severity: Severity;
  action: "allow" | "block";
  message: string;
  matched_paths: string[];
}

export type EvidenceLevel = "tokenizer_fingerprint" | "self_reported" | "undetermined";

export interface AuditSourceInfo {
  id: string;
  label: string;
  path: string;
  available: boolean;
  records: number;
}

export interface AuditAnomalyCategory {
  key: string;
  label: string;
  count: number;
}

export interface AuditBreakdownRow {
  requested_model: string;
  actual_model: string;
  evidence_level: EvidenceLevel;
  count: number;
}

export interface AuditReportRow {
  id: string;
  time: string;
  source: string;
  provider: string;
  requested_model: string;
  actual_model: string;
  evidence_level: EvidenceLevel;
  input_tokens?: number;
  output_tokens?: number;
  reasoning_tokens?: number;
  status_code?: number;
  categories: string[];
}

export interface DailyAudit {
  date: string;
  generated_at: string;
  sources: AuditSourceInfo[];
  total_requests: number;
  analyzed_requests: number;
  errors: number;
  anomaly_total: number;
  clean_requests: number;
  anomalies: AuditAnomalyCategory[];
  actual_model_breakdown: AuditBreakdownRow[];
  tokens: { input: number; output: number; total: number };
  rows: AuditReportRow[];
  limitations: string[];
}

export type RoutingVerdict = "match" | "mismatch" | "unknown" | "error";

export interface ModelRoutingResult {
  requested_model: string;
  reported_model?: string;
  verdict: RoutingVerdict;
  conflict: boolean;
  observed: string[];
  endpoint: string;
  http_status?: number;
  detail: string;
}
