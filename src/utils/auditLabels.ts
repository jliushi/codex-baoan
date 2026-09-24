import type { EvidenceLevel } from "../types/audit";

export const EVIDENCE_LABELS: Record<EvidenceLevel, string> = {
  self_reported: "上游自报",
  tokenizer_fingerprint: "分词器指纹",
  undetermined: "无法判定",
};

export const CATEGORY_LABELS: Record<string, string> = {
  routing_substitution: "模型路由或替换",
  token_anomaly: "Token 用量矛盾",
  invalid_model_accepted: "接受了无效模型名",
};

export function categoryLabel(key: string): string {
  return CATEGORY_LABELS[key] ?? key;
}
