import { useMemo, useState } from "react";
import { ListFilter, Search, X } from "lucide-react";
import type { EvidenceLevel, ReportRow } from "../types/audit";
import { CATEGORY_LABELS, EVIDENCE_LABELS, categoryLabel } from "../utils/auditLabels";
import { AnomalyTable } from "../components/audit/AnomalyTable";

interface RequestsPageProps {
  rows: ReportRow[];
  initialCategory: string;
  onCategoryChange: (category: string) => void;
  onSelectRow: (row: ReportRow) => void;
}

type EvidenceFilter = EvidenceLevel | "all";

export function RequestsPage({
  rows,
  initialCategory,
  onCategoryChange,
  onSelectRow,
}: RequestsPageProps) {
  const [query, setQuery] = useState("");
  const [evidence, setEvidence] = useState<EvidenceFilter>("all");

  const filteredRows = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase("zh-CN");
    return rows.filter((row) => {
      if (initialCategory !== "all" && !row.categories.includes(initialCategory)) {
        return false;
      }
      if (evidence !== "all" && row.evidence_level !== evidence) return false;
      if (!normalizedQuery) return true;

      const searchable = [
        row.time,
        row.provider,
        row.requested_model,
        row.actual_model,
        row.id,
        ...row.categories.map(categoryLabel),
      ]
        .join(" ")
        .toLocaleLowerCase("zh-CN");
      return searchable.includes(normalizedQuery);
    });
  }, [rows, initialCategory, evidence, query]);

  const hasFilters = query.length > 0 || evidence !== "all" || initialCategory !== "all";
  const clearFilters = () => {
    setQuery("");
    setEvidence("all");
    onCategoryChange("all");
  };

  return (
    <section className="panel requests-panel">
      <div className="panel-heading requests-heading">
        <div>
          <h2>异常请求记录</h2>
          <p>
            显示 {filteredRows.length} / {rows.length} 条记录
          </p>
        </div>
        <ListFilter className="panel-icon" size={18} aria-hidden="true" />
      </div>

      <div className="filter-toolbar">
        <label className="search-control">
          <Search size={16} aria-hidden="true" />
          <span className="sr-only">搜索异常请求</span>
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索模型、供应商或异常类型"
          />
          {query && (
            <button
              className="search-clear"
              type="button"
              onClick={() => setQuery("")}
              aria-label="清除搜索"
            >
              <X size={14} aria-hidden="true" />
            </button>
          )}
        </label>

        <label className="select-control">
          <span>异常类型</span>
          <select
            value={initialCategory}
            onChange={(event) => onCategoryChange(event.target.value)}
          >
            <option value="all">全部类型</option>
            {Object.entries(CATEGORY_LABELS).map(([key, label]) => (
              <option value={key} key={key}>{label}</option>
            ))}
          </select>
        </label>

        <label className="select-control evidence-filter">
          <span>证据级别</span>
          <select
            value={evidence}
            onChange={(event) => setEvidence(event.target.value as EvidenceFilter)}
          >
            <option value="all">全部证据</option>
            {Object.entries(EVIDENCE_LABELS).map(([key, label]) => (
              <option value={key} key={key}>{label}</option>
            ))}
          </select>
        </label>

        {hasFilters && (
          <button className="clear-filters" type="button" onClick={clearFilters}>
            清除筛选
          </button>
        )}
      </div>

      <AnomalyTable rows={filteredRows} onSelect={onSelectRow} />
    </section>
  );
}
