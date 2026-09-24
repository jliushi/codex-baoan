import { ArrowUpRight, CircleCheck, Radar } from "lucide-react";
import type { BreakdownRow } from "../../types/audit";
import { EvidenceBadge } from "./EvidenceBadge";

interface ModelBreakdownTableProps {
  rows: BreakdownRow[];
}

export function ModelBreakdownTable({ rows }: ModelBreakdownTableProps) {
  return (
    <section className="panel breakdown-panel">
      <div className="panel-heading">
        <div>
          <h2>模型差异</h2>
          <p>请求指定的模型与响应自报的模型</p>
        </div>
        <span className="panel-icon" aria-hidden="true">
          <Radar size={17} strokeWidth={1.8} />
        </span>
      </div>
      {rows.length > 0 ? (
        <div className="table-scroll">
          <table className="data-table breakdown-table">
            <thead>
              <tr>
                <th>请求模型</th>
                <th aria-label="差异方向" />
                <th>上游自报</th>
                <th>证据</th>
                <th className="align-right">次数</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row, index) => (
                <tr key={`${row.requested_model}-${row.actual_model}-${index}`}>
                  <td>
                    <code className="model-name">{row.requested_model}</code>
                  </td>
                  <td className="direction-cell">
                    <ArrowUpRight size={14} aria-hidden="true" />
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
                  <td className="align-right count-cell">{row.count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="table-empty">
          <CircleCheck size={19} aria-hidden="true" />
          <span>当天没有记录到模型名称差异</span>
        </div>
      )}
    </section>
  );
}
