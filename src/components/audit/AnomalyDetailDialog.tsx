import { useEffect } from "react";
import { CircleAlert, X } from "lucide-react";
import type { ReportRow } from "../../types/audit";
import { categoryLabel } from "../../utils/auditLabels";
import { EvidenceBadge } from "./EvidenceBadge";

interface AnomalyDetailDialogProps {
  row: ReportRow | null;
  onClose: () => void;
}

export function AnomalyDetailDialog({ row, onClose }: AnomalyDetailDialogProps) {
  useEffect(() => {
    if (!row) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [row, onClose]);

  if (!row) return null;

  return (
    <div
      className="dialog-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        className="detail-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="detail-title"
      >
        <div className="dialog-heading">
          <div className="dialog-heading-icon" aria-hidden="true">
            <CircleAlert size={19} />
          </div>
          <div>
            <p className="eyebrow">异常请求详情</p>
            <h2 id="detail-title">{row.time} · {row.provider}</h2>
          </div>
          <button
            className="icon-button dialog-close"
            type="button"
            onClick={onClose}
            aria-label="关闭详情"
          >
            <X size={17} aria-hidden="true" />
          </button>
        </div>

        <div className="model-comparison">
          <div className="comparison-cell">
            <span>请求模型</span>
            <code>{row.requested_model}</code>
          </div>
          <span className="comparison-arrow" aria-hidden="true">→</span>
          <div className="comparison-cell actual">
            <span>上游自报模型</span>
            <code>{row.actual_model}</code>
          </div>
        </div>

        <div className="detail-grid">
          <div className="detail-field">
            <span>证据级别</span>
            <EvidenceBadge level={row.evidence_level} />
          </div>
          <div className="detail-field">
            <span>HTTP 状态码</span>
            <strong>{row.status_code ?? "未记录"}</strong>
          </div>
          <div className="detail-field">
            <span>输入 Token</span>
            <strong>{row.input_tokens.toLocaleString("zh-CN")}</strong>
          </div>
          <div className="detail-field">
            <span>缓存读取 Token</span>
            <strong>{row.cache_read.toLocaleString("zh-CN")}</strong>
          </div>
        </div>

        <div className="dialog-section">
          <h3>异常类型</h3>
          <div className="detail-tags">
            {row.categories.map((category) => (
              <span className="category-chip" key={category}>
                {categoryLabel(category)}
              </span>
            ))}
          </div>
        </div>
        <p className="dialog-note">
          上游模型名称来自响应字段，不能证明实际运行的权重。完整请求正文不会在此视图中展示。
        </p>
      </section>
    </div>
  );
}
