import {
  Activity,
  ArrowRight,
  CircleAlert,
  CircleCheck,
  Database,
  Fingerprint,
  RotateCcw,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { AuditReport, ReportRow } from "../types/audit";
import { categoryLabel } from "../utils/auditLabels";
import { AnomalyTable } from "../components/audit/AnomalyTable";
import { MetricCard } from "../components/audit/MetricCard";
import { ModelBreakdownTable } from "../components/audit/ModelBreakdownTable";

interface OverviewPageProps {
  report: AuditReport;
  onCategorySelect: (category: string) => void;
  onShowAll: () => void;
  onSelectRow: (row: ReportRow) => void;
}

const CATEGORY_ICONS: Record<string, { icon: LucideIcon; tone: string }> = {
  routing_substitution: { icon: RotateCcw, tone: "amber" },
  token_anomaly: { icon: Activity, tone: "blue" },
  invalid_model_accepted: { icon: CircleAlert, tone: "red" },
};

export function OverviewPage({
  report,
  onCategorySelect,
  onShowAll,
  onSelectRow,
}: OverviewPageProps) {
  return (
    <div className="page-stack">
      <section className="metric-grid" aria-label="当日汇总">
        <MetricCard
          label="分析请求"
          value={report.total_requests}
          caption="CC Switch 记录的 Codex 请求"
          icon={Database}
          tone="blue"
        />
        <MetricCard
          label="异常请求"
          value={report.anomaly_total}
          caption="按请求 ID 去重后的异常数"
          icon={CircleAlert}
          tone="red"
        />
        <MetricCard
          label="失败请求"
          value={report.errors}
          caption="HTTP 4xx / 5xx 或状态码为 0"
          icon={Activity}
          tone="amber"
        />
        <MetricCard
          label="未命中异常"
          value={report.clean_requests}
          caption="在当前规则范围内未发现异常"
          icon={CircleCheck}
          tone="green"
        />
      </section>

      <section className="section-block">
        <div className="section-heading">
          <div>
            <h2>异常分类</h2>
            <p>选择一种类型，查看对应的请求记录</p>
          </div>
          <span className="section-meta">{report.anomaly_total} 条异常</span>
        </div>
        <div className="category-grid">
          {report.anomalies.map((category) => {
            const visual = CATEGORY_ICONS[category.key] ?? {
              icon: Fingerprint,
              tone: "blue",
            };
            const Icon = visual.icon;
            return (
              <button
                className="category-card"
                type="button"
                key={category.key}
                onClick={() => onCategorySelect(category.key)}
              >
                <span className={`category-icon tone-${visual.tone}`}>
                  <Icon size={17} strokeWidth={1.9} aria-hidden="true" />
                </span>
                <span className="category-copy">
                  <strong>{categoryLabel(category.key)}</strong>
                  <small>{category.label}</small>
                </span>
                <span className="category-count">{category.count}</span>
                <ArrowRight className="category-arrow" size={15} aria-hidden="true" />
              </button>
            );
          })}
        </div>
      </section>

      <section className="overview-grid">
        <ModelBreakdownTable rows={report.actual_model_breakdown} />
        <aside className="panel scope-panel">
          <div className="panel-heading">
            <div>
              <h2>审计范围</h2>
              <p>当前数据来源与判定边界</p>
            </div>
            <span className="panel-icon" aria-hidden="true">
              <Fingerprint size={17} strokeWidth={1.8} />
            </span>
          </div>
          <div className="scope-status">
            <span className="status-indicator status-good" aria-hidden="true" />
            <span>只读接入 CC Switch 本机数据库</span>
          </div>
          <p className="scope-copy">
            页面统计请求路由、缓存 Token 矛盾和无效模型名。上游自报字段仅作线索，不等同于模型权重认证。
          </p>
          <ul className="scope-list">
            {report.limitations.slice(0, 2).map((limitation) => (
              <li key={limitation}>{limitation}</li>
            ))}
          </ul>
        </aside>
      </section>

      <section className="panel recent-panel">
        <div className="panel-heading recent-heading">
          <div>
            <h2>最近异常</h2>
            <p>按发生时间倒序排列</p>
          </div>
          <button className="text-action" type="button" onClick={onShowAll}>
            查看全部 <ArrowRight size={14} aria-hidden="true" />
          </button>
        </div>
        <AnomalyTable
          rows={report.rows.slice(0, 6)}
          compact
          onSelect={onSelectRow}
        />
      </section>
    </div>
  );
}
