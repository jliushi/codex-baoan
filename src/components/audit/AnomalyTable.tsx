import { ArrowUpRight, FileSearch } from "lucide-react";
import type { ReportRow } from "../../types/audit";
import { categoryLabel } from "../../utils/auditLabels";
import { EvidenceBadge } from "./EvidenceBadge";

interface AnomalyTableProps {
  rows: ReportRow[];
  compact?: boolean;
  onSelect: (row: ReportRow) => void;
}

export function AnomalyTable({ rows, compact = false, onSelect }: AnomalyTableProps) {
  const columns = compact ? 6 : 8;

  return (
    <div className="table-scroll">
      <table className={`data-table anomaly-table${compact ? " compact" : ""}`}>
        <thead>
          <tr>
            <th>时间</th>
            <th>供应商</th>
            <th>请求模型</th>
            <th>上游自报</th>
            <th>证据</th>
            {!compact && <th>异常类型</th>}
            {!compact && <th className="align-right">状态码</th>}
            <th aria-label="查看详情" />
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.id}>
              <td className="time-cell">{row.time}</td>
              <td className="provider-cell">{row.provider}</td>
              <td>
                <code className="model-name">{row.requested_model}</code>
              </td>
              <td>
                <code
                  className={`model-name ${row.evidence_level === "undetermined" ? "text-muted" : "text-risk"}`}
                >
                  {row.actual_model}
                </code>
              </td>
              <td>
                <EvidenceBadge level={row.evidence_level} />
              </td>
              {!compact && (
                <td>
                  <div className="category-list">
                    {row.categories.map((category) => (
                      <span className="category-chip" key={category}>
                        {categoryLabel(category)}
                      </span>
                    ))}
                  </div>
                </td>
              )}
              {!compact && (
                <td className="align-right status-cell">
                  {row.status_code ?? "—"}
                </td>
              )}
              <td className="action-cell">
                <button
                  className="row-action"
                  type="button"
                  onClick={() => onSelect(row)}
                  aria-label={`查看 ${row.time} 的异常请求详情`}
                  title="查看详情"
                >
                  {compact ? (
                    <ArrowUpRight size={15} aria-hidden="true" />
                  ) : (
                    <FileSearch size={15} aria-hidden="true" />
                  )}
                </button>
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td className="table-empty-cell" colSpan={columns}>
                当天没有符合条件的异常请求。
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
