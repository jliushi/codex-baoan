import { useState } from "react";
import { AlertTriangle, Database, RefreshCw } from "lucide-react";
import { AppHeader } from "./components/layout/AppHeader";
import { AnomalyDetailDialog } from "./components/audit/AnomalyDetailDialog";
import { EvidencePage } from "./pages/EvidencePage";
import { OverviewPage } from "./pages/OverviewPage";
import { RequestsPage } from "./pages/RequestsPage";
import { useTheme } from "./contexts/ThemeContext";
import { useAuditReport } from "./hooks/useAuditReport";
import type { AuditPage, ReportRow } from "./types/audit";

function todayInShanghai(): string {
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(new Date());
  const part = (type: string) => parts.find((item) => item.type === type)?.value ?? "";
  return `${part("year")}-${part("month")}-${part("day")}`;
}

const PAGE_META: Record<AuditPage, { title: string; subtitle: string }> = {
  overview: {
    title: "审计概览",
    subtitle: "汇总请求情况、异常类型和模型名称差异。所有日期均按北京时间统计。",
  },
  requests: {
    title: "异常请求",
    subtitle: "搜索和筛选命中的请求记录，查看每条记录的证据与用量摘要。",
  },
  evidence: {
    title: "判定说明",
    subtitle: "了解每种证据能说明什么，以及当前日志分析的边界。",
  },
};

function PageTitle({
  page,
  available,
  loading,
  generatedAt,
}: {
  page: AuditPage;
  available: boolean;
  loading: boolean;
  generatedAt?: string;
}) {
  const meta = PAGE_META[page];
  const status = loading && !available ? "正在读取" : available ? "本机只读数据" : "未连接数据";

  return (
    <div className="page-title-row">
      <div className="page-title-copy">
        <p className="eyebrow">LOCAL MODEL AUDIT</p>
        <h1>{meta.title}</h1>
        <p className="page-subtitle">{meta.subtitle}</p>
      </div>
      <div className={`source-pill ${available ? "source-connected" : "source-disconnected"}`}>
        <span className={`status-indicator ${available ? "status-good" : loading ? "status-loading" : "status-idle"}`} />
        <span className="source-copy">
          <strong>CC Switch</strong>
          <small>{generatedAt ? `更新于 ${generatedAt}` : status}</small>
        </span>
      </div>
    </div>
  );
}

function LoadingState() {
  return (
    <section className="panel loading-panel" aria-live="polite">
      <span className="loading-spinner" aria-hidden="true" />
      <div>
        <strong>正在读取本机审计数据</strong>
        <p>从 CC Switch 数据库载入当天的 Codex 请求记录。</p>
      </div>
    </section>
  );
}

function SourceUnavailable({
  onRefresh,
  loading,
  reason,
}: {
  onRefresh: () => void;
  loading: boolean;
  reason?: string;
}) {
  return (
    <section className="panel source-empty">
      <span className="empty-icon" aria-hidden="true"><Database size={22} /></span>
      <p className="eyebrow">等待本机数据</p>
      <h2>无法读取 CC Switch 请求数据库</h2>
      <p>
        请启动 CC Switch 并通过 Codex 发送请求，确认本机数据库可读取后点击刷新。
      </p>
      {reason && <code className="source-reason">{reason}</code>}
      <button className="button button-primary" type="button" onClick={onRefresh} disabled={loading}>
        <RefreshCw size={15} className={loading ? "spin" : undefined} aria-hidden="true" />
        重新读取
      </button>
    </section>
  );
}

export default function App() {
  const [date, setDate] = useState(todayInShanghai);
  const [page, setPage] = useState<AuditPage>("overview");
  const [categoryFilter, setCategoryFilter] = useState("all");
  const [selectedRow, setSelectedRow] = useState<ReportRow | null>(null);
  const { report, loading, error, refresh } = useAuditReport(date);
  const { theme, toggleTheme } = useTheme();

  // Do not briefly show yesterday's result beneath a newly selected date.
  const visibleReport = report?.date === date ? report : null;
  const sourceAvailable = visibleReport?.source_available === true;
  const loadError = Boolean(error && !visibleReport);

  const openRequestsForCategory = (category: string) => {
    setCategoryFilter(category);
    setPage("requests");
  };

  const generatedAt = visibleReport?.generated_at
    ? new Intl.DateTimeFormat("zh-CN", {
        timeZone: "Asia/Shanghai",
        hour: "2-digit",
        minute: "2-digit",
      }).format(new Date(visibleReport.generated_at))
    : undefined;

  return (
    <div className="app-shell">
      <AppHeader
        page={page}
        onPageChange={setPage}
        date={date}
        onDateChange={(nextDate) => {
          if (nextDate) {
            setSelectedRow(null);
            setDate(nextDate);
          }
        }}
        maxDate={todayInShanghai()}
        loading={loading}
        onRefresh={() => void refresh()}
        theme={theme}
        onToggleTheme={toggleTheme}
      />

      <main className="main-content">
        <div className="content-inner">
          <PageTitle
            page={page}
            available={sourceAvailable}
            loading={loading}
            generatedAt={sourceAvailable ? generatedAt : undefined}
          />

          {error && (
            <div className="alert-banner" role="alert">
              <AlertTriangle size={17} aria-hidden="true" />
              <span>{error}</span>
              <button type="button" onClick={() => void refresh()} disabled={loading}>
                重试
              </button>
            </div>
          )}

          {loading && !visibleReport ? (
            <LoadingState />
          ) : loadError && page !== "evidence" ? (
            <section className="panel source-empty">
              <span className="empty-icon empty-icon-warning" aria-hidden="true"><AlertTriangle size={22} /></span>
              <p className="eyebrow">数据读取失败</p>
              <h2>暂时无法载入审计数据</h2>
              <p>{error}</p>
              <button className="button button-primary" type="button" onClick={() => void refresh()} disabled={loading}>
                <RefreshCw size={15} className={loading ? "spin" : undefined} aria-hidden="true" />
                重试
              </button>
            </section>
          ) : page === "evidence" ? (
            <EvidencePage report={visibleReport} />
          ) : visibleReport && !visibleReport.source_available ? (
            <SourceUnavailable
              onRefresh={() => void refresh()}
              loading={loading}
              reason={visibleReport.limitations[0]}
            />
          ) : visibleReport && page === "overview" ? (
            <OverviewPage
              report={visibleReport}
              onCategorySelect={openRequestsForCategory}
              onShowAll={() => {
                setCategoryFilter("all");
                setPage("requests");
              }}
              onSelectRow={setSelectedRow}
            />
          ) : visibleReport ? (
            <RequestsPage
              rows={visibleReport.rows}
              initialCategory={categoryFilter}
              onCategoryChange={setCategoryFilter}
              onSelectRow={setSelectedRow}
            />
          ) : (
            <LoadingState />
          )}

          <footer className="app-footer">
            <span><span className="footer-dot" />数据保留在本机</span>
            <span>Codex 保安 · {date}</span>
          </footer>
        </div>
      </main>

      <AnomalyDetailDialog row={selectedRow} onClose={() => setSelectedRow(null)} />
    </div>
  );
}
