import {
  AlertTriangle,
  BadgeCheck,
  CircleHelp,
  Fingerprint,
  ShieldAlert,
} from "lucide-react";
import type { AuditReport, EvidenceLevel } from "../types/audit";
import { EVIDENCE_LABELS } from "../utils/auditLabels";
import { EvidenceBadge } from "../components/audit/EvidenceBadge";

interface EvidencePageProps {
  report: AuditReport | null;
}

const EVIDENCE_DETAILS: Array<{
  level: EvidenceLevel;
  icon: typeof BadgeCheck;
  description: string;
}> = [
  {
    level: "self_reported",
    icon: BadgeCheck,
    description:
      "模型名取自上游响应中的 model 字段。它能说明服务端返回了什么名称，但服务端可以改写这个字段。",
  },
  {
    level: "tokenizer_fingerprint",
    icon: Fingerprint,
    description:
      "通过请求 Token 数与已知分词器的计数结果比较，推断模型家族。它是统计线索，不能证明具体权重。",
  },
  {
    level: "undetermined",
    icon: CircleHelp,
    description:
      "现有日志不足以推断实际模型，或该异常与模型名称无关。页面会明确显示无法判定。",
  },
];

export function EvidencePage({ report }: EvidencePageProps) {
  return (
    <div className="page-stack evidence-page">
      <section className="panel evidence-intro">
        <div className="intro-icon" aria-hidden="true">
          <ShieldAlert size={21} strokeWidth={1.8} />
        </div>
        <div>
          <p className="eyebrow">不要把推断当成证明</p>
          <h2>审计结果会标出证据强度</h2>
          <p>
            模型服务端可以修改响应内容。Codex 保安将观察到的字段和统计推断分开呈现，不把模型自报名称包装成权重认证。
          </p>
        </div>
      </section>

      <section className="evidence-grid" aria-label="证据级别说明">
        {EVIDENCE_DETAILS.map(({ level, icon: Icon, description }) => (
          <article className="panel evidence-card" key={level}>
            <div className="evidence-card-top">
              <span className={`evidence-icon evidence-icon-${level}`}>
                <Icon size={18} strokeWidth={1.8} aria-hidden="true" />
              </span>
              <EvidenceBadge level={level} />
            </div>
            <h3>{EVIDENCE_LABELS[level]}</h3>
            <p>{description}</p>
          </article>
        ))}
      </section>

      <section className="panel limitations-panel">
        <div className="panel-heading">
          <div>
            <h2>当前版本的范围</h2>
            <p>读取本机 CC Switch Codex 请求日志</p>
          </div>
          <span className="panel-icon" aria-hidden="true">
            <AlertTriangle size={17} strokeWidth={1.8} />
          </span>
        </div>
        <ul className="limitation-list">
          {(report?.limitations.length
            ? report.limitations
            : [
                "模型自报字段不是权重认证。",
                "同名偷换无法仅靠响应中的模型名识别。",
                "当前 Rust 审计引擎尚未接入 MITM 分词器指纹采集。",
              ]
          ).map((limitation) => (
            <li key={limitation}>{limitation}</li>
          ))}
        </ul>
      </section>
    </div>
  );
}
