export type EvidenceLevel =
  | "self_reported"
  | "tokenizer_fingerprint"
  | "undetermined";

export interface AnomalyCategory {
  key: string;
  label: string;
  count: number;
}

export interface BreakdownRow {
  requested_model: string;
  actual_model: string;
  evidence_level: EvidenceLevel;
  count: number;
}

export interface ReportRow {
  id: string;
  time: string;
  provider: string;
  requested_model: string;
  actual_model: string;
  evidence_level: EvidenceLevel;
  input_tokens: number;
  cache_read: number;
  status_code: number | null;
  categories: string[];
}

export interface AuditReport {
  date: string;
  generated_at: string;
  source_available: boolean;
  total_requests: number;
  errors: number;
  anomaly_total: number;
  clean_requests: number;
  anomalies: AnomalyCategory[];
  actual_model_breakdown: BreakdownRow[];
  rows: ReportRow[];
  limitations: string[];
}

export type AuditPage = "overview" | "requests" | "evidence";

export interface GuardStatus {
  cert_trusted: boolean;
  ccswitch_found: boolean;
  attached: boolean;
  provider: string;
  proxy_url: string;
}

export interface GroupVerdict {
  requested_model: string;
  reported_models: string[];
  samples: number;
  family: string;
  family_label: string;
  slope: number;
  rmse: number;
  note: string;
}
