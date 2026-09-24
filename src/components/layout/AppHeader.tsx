import {
  CircleHelp,
  LayoutDashboard,
  ListFilter,
  Moon,
  RefreshCw,
  ShieldCheck,
  Sun,
} from "lucide-react";
import type { AuditPage } from "../../types/audit";
import type { Theme } from "../../contexts/ThemeContext";

interface AppHeaderProps {
  page: AuditPage;
  onPageChange: (page: AuditPage) => void;
  date: string;
  onDateChange: (date: string) => void;
  maxDate: string;
  loading: boolean;
  onRefresh: () => void;
  theme: Theme;
  onToggleTheme: () => void;
}

const NAV_ITEMS = [
  { id: "overview", label: "审计概览", icon: LayoutDashboard },
  { id: "requests", label: "异常请求", icon: ListFilter },
  { id: "evidence", label: "判定说明", icon: CircleHelp },
] as const;

export function AppHeader({
  page,
  onPageChange,
  date,
  onDateChange,
  maxDate,
  loading,
  onRefresh,
  theme,
  onToggleTheme,
}: AppHeaderProps) {
  const ThemeIcon = theme === "dark" ? Sun : Moon;

  return (
    <header className="app-header">
      <div className="brand-lockup">
        <span className="brand-mark" aria-hidden="true">
          <ShieldCheck size={19} strokeWidth={2.1} />
        </span>
        <span className="brand-text">
          <strong>Codex 保安</strong>
          <small>本机模型审计</small>
        </span>
      </div>

      <nav className="primary-nav" aria-label="主导航">
        {NAV_ITEMS.map(({ id, label, icon: Icon }) => (
          <button
            className={`nav-item${page === id ? " active" : ""}`}
            type="button"
            key={id}
            aria-current={page === id ? "page" : undefined}
            onClick={() => onPageChange(id)}
          >
            <Icon size={16} strokeWidth={1.9} aria-hidden="true" />
            <span>{label}</span>
          </button>
        ))}
      </nav>

      <div className="header-tools">
        <label className="date-control">
          <span className="sr-only">选择审计日期</span>
          <input
            type="date"
            value={date}
            max={maxDate}
            onChange={(event) => onDateChange(event.target.value)}
          />
        </label>
        <button
          className="button button-primary refresh-button"
          type="button"
          onClick={onRefresh}
          disabled={loading}
        >
          <RefreshCw
            size={15}
            className={loading ? "spin" : undefined}
            aria-hidden="true"
          />
          <span>{loading ? "读取中" : "刷新"}</span>
        </button>
        <span className="toolbar-divider" aria-hidden="true" />
        <button
          className="icon-button"
          type="button"
          onClick={onToggleTheme}
          title={theme === "dark" ? "切换到浅色主题" : "切换到深色主题"}
          aria-label={theme === "dark" ? "切换到浅色主题" : "切换到深色主题"}
        >
          <ThemeIcon size={17} strokeWidth={1.9} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}
