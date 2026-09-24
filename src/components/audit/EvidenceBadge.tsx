import type { EvidenceLevel } from "../../types/audit";
import { EVIDENCE_LABELS } from "../../utils/auditLabels";

interface EvidenceBadgeProps {
  level: EvidenceLevel;
}

export function EvidenceBadge({ level }: EvidenceBadgeProps) {
  return (
    <span className={`evidence-badge evidence-${level}`}>
      <span className="evidence-dot" aria-hidden="true" />
      {EVIDENCE_LABELS[level]}
    </span>
  );
}
