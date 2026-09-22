import { invoke } from "@tauri-apps/api/core";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  Activity,
  AlertTriangle,
  ArrowUpCircle,
  Check,
  Download,
  ExternalLink,
  Eye,
  EyeOff,
  FileMinus,
  FilePen,
  FilePlus,
  FileStack,
  Fingerprint,
  FolderOpen,
  Globe,
  Layers,
  Loader2,
  Minimize2,
  Monitor,
  Moon,
  Play,
  Power,
  RefreshCw,
  Search,
  Settings,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Square,
  Sun,
  Terminal,
  Trash2,
  X
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ActivityEvent, ActivityFilter, ActivityKind, AppState, DailyAudit, DiscoveredProvider, EvidenceLevel, GuardMode, ModelRoutingResult, RoutingVerdict } from "./types";
import ccswitchIcon from "./assets/ccswitch.png";

// 北京时间 (UTC+08:00) 当天日期，格式 YYYY-MM-DD，用于日报默认日期
function todayShanghai(): string {
  return new Intl.DateTimeFormat("en-CA", { timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit" }).format(new Date());
}

const EVIDENCE_LABEL: Record<EvidenceLevel, string> = {
  tokenizer_fingerprint: "分词器指纹",
  self_reported: "中转自报",
  undetermined: "无法判定"
};

type Theme = "light" | "dark" | "system";
type KpiTone = "primary" | "neutral" | "danger" | "calm";
type UpdateState = { status: "idle" | "checking" | "latest" | "available" | "downloading" | "error"; latest?: string; message?: string; progress?: number };
type Settings = { background_run: boolean; silent_start: boolean; autostart: boolean };

const THEME_KEY = "cgx-theme";

const emptyState: AppState = {
  app: { version: "0.2.3", install_dir: "", sessions_dir: "", bundle_managed: false, updater_configured: false, portable_mode: false },
  discovery: { generated_at: "", providers: [], sources: [], manual_fallback_reason: "正在读取本机配置…" },
  runtime: { running: false, mode: "audit" },
  activity: []
};

const filters: Array<{ id: ActivityFilter; label: string }> = [
  { id: "all", label: "全部活动" },
  { id: "command", label: "命令" },
  { id: "file-read", label: "读取" },
  { id: "file-create", label: "新建" },
  { id: "file-modify", label: "修改" },
  { id: "file-delete", label: "删除" },
  { id: "network", label: "网络" },
  { id: "risk", label: "高危" }
];

function readTheme(): Theme {
  if (typeof localStorage === "undefined") return "system";
  const saved = localStorage.getItem(THEME_KEY);
  return saved === "light" || saved === "dark" || saved === "system" ? saved : "system";
}

function verdictLabel(verdict: RoutingVerdict): string {
  switch (verdict) {
    case "match": return "自报一致";
    case "mismatch": return "被路由/替换";
    case "unknown": return "上游未自报";
    default: return "检测失败";
  }
}

export function App() {
  const [state, setState] = useState<AppState>(emptyState);
  const [activityFilter, setActivityFilter] = useState<ActivityFilter>("all");
  const [search, setSearch] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [mode] = useState<GuardMode>("audit");
  const [theme, setTheme] = useState<Theme>(readTheme);
  const [update, setUpdate] = useState<UpdateState>({ status: "idle" });
  const [settings, setSettings] = useState<Settings>({ background_run: true, silent_start: false, autostart: false });
  const [toast, setToast] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const [managementBusy, setManagementBusy] = useState<string | null>(null);
  const [probeResult, setProbeResult] = useState<ModelRoutingResult | null>(null);
  const [probing, setProbing] = useState(false);
  const [report, setReport] = useState<DailyAudit | null>(null);
  const [reportDate, setReportDate] = useState<string>(todayShanghai);
  const [reportLoading, setReportLoading] = useState(false);

  const drawerCloseRef = useRef<HTMLButtonElement>(null);
  const settingsButtonRef = useRef<HTMLButtonElement>(null);
  const toastTimerRef = useRef<number | null>(null);
  const railRef = useRef<HTMLElement>(null);
  const stageRef = useRef<HTMLElement>(null);
  const updateAutoCheckRef = useRef(false);

  const runProbe = useCallback(async () => {
    setProbing(true);
    setError("");
    try {
      // 不传参：后端读本机 Codex 配置(base_url + 明文 key + model)自动探测
      const result = await invoke<ModelRoutingResult>("detect_model_routing", {});
      setProbeResult(result);
    } catch (err) {
      setError(typeof err === "string" ? err : "模型检测失败");
    } finally {
      setProbing(false);
    }
  }, []);

  const loadReport = useCallback(async (date: string) => {
    setReportLoading(true);
    setError("");
    try {
      const result = await invoke<DailyAudit>("audit_daily_report", { date });
      setReport(result);
    } catch (err) {
      setError(typeof err === "string" ? err : "读取审计日报失败");
    } finally {
      setReportLoading(false);
    }
  }, []);

  useEffect(() => {
    loadReport(reportDate);
  }, [reportDate, loadReport]);

  const activeUpstream = useMemo(
    () => state.discovery.providers.find((item) => item.id === state.discovery.recommended_provider_id) || state.discovery.providers[0],
    [state.discovery.providers, state.discovery.recommended_provider_id]
  );

  const filteredActivity = useMemo(() => {
    let list =
      activityFilter === "all"
        ? state.activity
        : activityFilter === "risk"
          ? state.activity.filter(isRisk)
          : state.activity.filter((event) => event.kind === activityFilter);
    const q = search.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (event) =>
          (event.command || "").toLowerCase().includes(q) ||
          event.title.toLowerCase().includes(q) ||
          (event.source || "").toLowerCase().includes(q) ||
          event.summary.toLowerCase().includes(q) ||
          event.paths.some((path) => path.toLowerCase().includes(q))
      );
    }
    return list;
  }, [activityFilter, state.activity, search]);

  const stats = useMemo(() => {
    const count = (kind: ActivityKind) => state.activity.filter((event) => event.kind === kind).length;
    const riskCount = state.activity.filter((event) => event.kind === "risk" || event.severity === "high" || event.severity === "critical").length;
    return {
      total: state.activity.length,
      commands: count("command"),
      reads: count("file-read"),
      creates: count("file-create"),
      modifies: count("file-modify"),
      deletes: count("file-delete"),
      network: count("network"),
      files: count("file-read") + count("file-create") + count("file-modify") + count("file-delete"),
      risks: riskCount
    };
  }, [state.activity]);

  const notify = useCallback((message: string) => {
    if (toastTimerRef.current) window.clearTimeout(toastTimerRef.current);
    setToast(message);
    toastTimerRef.current = window.setTimeout(() => {
      setToast("");
      toastTimerRef.current = null;
    }, 2400);
  }, []);

  const fail = useCallback((err: unknown) => {
    setError(typeof err === "string" ? err : err instanceof Error ? err.message : String(err));
  }, []);

  const refresh = useCallback(
    async (announce?: string) => {
      setScanning(true);
      setError("");
      try {
        const next = await invoke<AppState>("get_state");
        setState(next);
        if (announce) notify(announce);
      } catch (err) {
        fail(err);
      } finally {
        setScanning(false);
        setLoading(false);
      }
    },
    [fail, notify]
  );

  async function start() {
    setActionBusy(true);
    setError("");
    try {
      const next = await invoke<AppState>("start_guard", { mode });
      setState(next);
      notify(`监控已启动 · ${next.runtime.provider_name || "本机会话"}`);
    } catch (err) {
      fail(err);
    } finally {
      setActionBusy(false);
    }
  }

  async function stop() {
    setActionBusy(true);
    setError("");
    try {
      const next = await invoke<AppState>("stop_guard");
      setState(next);
      notify("监控已停止");
    } catch (err) {
      fail(err);
    } finally {
      setActionBusy(false);
    }
  }

  async function clearActivity() {
    setError("");
    try {
      const activity = await invoke<ActivityEvent[]>("clear_activity");
      setState((current) => ({ ...current, activity }));
      notify("监控记录已清空");
    } catch (err) {
      fail(err);
    }
  }

  async function runManaged(id: string, message: string, action: () => Promise<unknown>) {
    setManagementBusy(id);
    setError("");
    try {
      await action();
      notify(message);
    } catch (err) {
      fail(err);
    } finally {
      setManagementBusy(null);
    }
  }

  const checkUpdate = useCallback(async () => {
    if (!state.app.updater_configured) {
      setUpdate({
        status: "error",
        message: state.app.portable_mode ? "当前为便携版，应用内更新不可用，请打开下载页下载安装包" : "开发模式不执行应用内更新，请打开下载页查看正式版本"
      });
      return;
    }
    setUpdate({ status: "checking" });
    try {
      const upd = await check({ timeout: 30000 });
      if (upd) {
        setUpdate({ status: "available", latest: upd.version, message: "发现新版本，可直接下载安装" });
      } else {
        setUpdate({ status: "latest", latest: state.app.version, message: `已是最新版本 (v${state.app.version})` });
      }
    } catch (err) {
      const detail = err instanceof Error ? err.message : typeof err === "string" ? err : "更新服务暂不可用";
      setUpdate({ status: "error", message: `检查更新失败：${detail}。可打开下载页手动安装。` });
    }
  }, [state.app.portable_mode, state.app.updater_configured, state.app.version]);

  const installUpdate = useCallback(async () => {
    if (!state.app.updater_configured) {
      setUpdate({
        status: "error",
        message: state.app.portable_mode ? "当前为便携版，应用内更新不可用，请打开下载页下载安装包" : "开发模式不执行应用内更新，请打开下载页查看正式版本"
      });
      return;
    }
    setUpdate((current) => ({ ...current, status: "downloading", progress: 0 }));
    try {
      const upd = await check({ timeout: 30000 });
      if (!upd) {
        setUpdate({ status: "latest", latest: state.app.version, message: `已是最新版本 (v${state.app.version})` });
        return;
      }
      let total = 0;
      let done = 0;
      await upd.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          done += event.data.chunkLength;
          setUpdate({ status: "downloading", latest: upd.version, progress: total ? Math.min(100, Math.round((done / total) * 100)) : 0 });
        } else if (event.event === "Finished") {
          setUpdate({ status: "downloading", latest: upd.version, progress: 100, message: "更新已安装，正在重启…" });
        }
      });
      await relaunch();
    } catch (err) {
      const detail = err instanceof Error ? err.message : typeof err === "string" ? err : "更新下载或安装失败";
      setUpdate({ status: "error", message: `${detail}。可打开下载页手动安装。` });
    }
  }, [state.app.portable_mode, state.app.updater_configured, state.app.version]);

  async function updateSetting(patch: Partial<Settings>) {
    const previous = settings;
    const next = { ...settings, ...patch };
    setSettings(next);
    try {
      const applied = await invoke<Settings>("set_settings", next);
      setSettings(applied);
    } catch (err) {
      setSettings(previous);
      fail(err);
    }
  }

  const closeSettings = useCallback(() => {
    setSettingsOpen(false);
    window.setTimeout(() => settingsButtonRef.current?.focus(), 0);
  }, []);

  useEffect(() => {
    refresh().catch(fail);
  }, [fail, refresh]);

  useEffect(() => {
    if (loading || updateAutoCheckRef.current || !state.app.updater_configured) return;
    updateAutoCheckRef.current = true;
    const timer = window.setTimeout(() => {
      checkUpdate().catch(fail);
    }, 1000);
    return () => window.clearTimeout(timer);
  }, [checkUpdate, fail, loading, state.app.updater_configured]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      if (!settingsOpen) refresh().catch(fail);
    }, 1800);
    return () => window.clearInterval(timer);
  }, [fail, refresh, settingsOpen]);

  useEffect(() => {
    return () => {
      if (toastTimerRef.current) window.clearTimeout(toastTimerRef.current);
    };
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => root.classList.toggle("dark", theme === "dark" || (theme === "system" && mq.matches));
    apply();
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      /* ignore */
    }
    if (theme === "system") {
      mq.addEventListener("change", apply);
      return () => mq.removeEventListener("change", apply);
    }
  }, [theme]);

  useEffect(() => {
    for (const node of [railRef.current, stageRef.current]) {
      if (!node) continue;
      if (settingsOpen) node.setAttribute("inert", "");
      else node.removeAttribute("inert");
    }
  }, [settingsOpen]);

  useEffect(() => {
    if (!settingsOpen) return;
    if (update.status === "idle") checkUpdate();
    invoke<Settings>("get_settings").then(setSettings).catch(() => {});
    window.setTimeout(() => drawerCloseRef.current?.focus(), 0);
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeSettings();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [closeSettings, settingsOpen, update.status, checkUpdate]);

  const running = state.runtime.running;
  const primaryActionLabel = running ? "停止 Codex 监控" : "启动 Codex 监控";
  const autostartDisabled = !state.app.bundle_managed && !settings.autostart;
  const autostartHint = state.app.bundle_managed
    ? "开机时自动启动 Codex 保安"
    : settings.autostart
      ? "当前启动项来自非安装版，可关闭以避免残留"
      : "仅安装版可开启，避免开发/便携版留下启动项";

  return (
    <main className="app">
      <aside className="rail" ref={railRef}>
        <header className="brand">
          <span className="brand__mark" aria-hidden="true">
            <ShieldCheck size={20} strokeWidth={2.5} />
          </span>
          <span className="brand__text">
            <strong>Codex 保安</strong>
            <small>本机监控 · v{state.app.version}</small>
          </span>
        </header>

        <nav className="filters" aria-label="按活动类型筛选">
          <span className="railLabel filters__label">活动筛选</span>
          {filters.map((item) => {
            const active = activityFilter === item.id;
            return (
              <button key={item.id} className={["filter", active ? "is-active" : ""].join(" ")} aria-pressed={active} onClick={() => setActivityFilter(item.id)}>
                <span className={["filter__icon", item.id].join(" ")} aria-hidden="true">
                  <ActivityKindIcon kind={item.id} size={15} />
                </span>
                <span className="filter__label">{item.label}</span>
                <span className="filter__count">{filterCount(item.id, state.activity)}</span>
              </button>
            );
          })}
        </nav>

        <footer className="rail__foot">
          <div className="modePill">
            <Eye size={14} />
            <span>
              <small>保护策略</small>
              <strong>{running ? "监控记录中" : "未启用"}</strong>
            </span>
          </div>
          <button
            ref={settingsButtonRef}
            className="btn btn--ghost settingsBtn"
            aria-haspopup="dialog"
            aria-expanded={settingsOpen}
            aria-label="打开应用设置"
            onClick={() => setSettingsOpen(true)}
          >
            <Settings size={16} />
            设置
          </button>
        </footer>
      </aside>

      <section className="stage" ref={stageRef}>
        <header className="topbar">
          <UpstreamBar loading={loading} upstream={activeUpstream} sources={state.discovery.sources} running={running} fallback={state.discovery.manual_fallback_reason} />
          <div className="topbar__actions">
            <button className="btn btn--soft" title="切换供应商后点此重新扫描本机配置" aria-label="重新扫描本机配置" disabled={scanning || loading} onClick={() => refresh("已重新扫描本机配置")}>
              <RefreshCw size={15} className={scanning ? "spin" : ""} />
              重新扫描
            </button>
            <button className={["btn", running ? "btn--stop" : "btn--primary"].join(" ")} disabled={actionBusy || loading} aria-label={primaryActionLabel} onClick={() => (running ? stop() : start())}>
              {actionBusy ? <Loader2 size={16} className="spin" /> : running ? <Square size={14} fill="currentColor" /> : <Play size={14} fill="currentColor" />}
              {actionBusy ? "处理中…" : running ? "停止监控" : "启动监控"}
            </button>
          </div>
        </header>

        {error && (
          <div className="banner banner--error" role="alert">
            <AlertTriangle size={17} />
            <p>{error}</p>
            <button className="banner__close" aria-label="关闭提示" onClick={() => setError("")}>
              <X size={15} />
            </button>
          </div>
        )}

        <section className="kpis" aria-label="监控概览">
          <Kpi tone="primary" icon={<Activity size={17} />} label="活动事件" value={loading ? "—" : stats.total} detail={`${stats.commands} 命令 · ${stats.network} 网络`} active={activityFilter === "all"} onClick={() => { setActivityFilter("all"); setSearch(""); }} />
          <Kpi tone="neutral" icon={<FileStack size={17} />} label="文件改动" value={loading ? "—" : stats.files} detail={`读 ${stats.reads} · 改 ${stats.modifies} · 删 ${stats.deletes}`} active={activityFilter === "file-read"} onClick={() => { setActivityFilter("file-read"); setSearch(""); }} />
          <Kpi tone={stats.risks ? "danger" : "calm"} icon={<ShieldAlert size={17} />} label="风险命中" value={loading ? "—" : stats.risks} detail="点击查看高危记录" active={activityFilter === "risk"} onClick={() => { setActivityFilter("risk"); setSearch(""); }} />
        </section>

        <section className="probe" aria-label="模型体检">
          <div className="probe__head">
            <div className="probe__headText">
              <h2><Fingerprint size={16} /> 模型体检</h2>
              <p>向当前上游发一次探测，读它自报的真实模型，识别「请求 A 却被路由/替换成 B」的掺水。上游若把 model 字段改干净则读不出（方法固有边界）。</p>
            </div>
            <button className="btn btn--primary btn--sm" disabled={probing || loading} aria-label="检测模型" onClick={runProbe}>
              {probing ? <Loader2 size={14} className="spin" /> : <Fingerprint size={14} />}
              {probing ? "检测中…" : "检测模型"}
            </button>
          </div>
          {probeResult && (
            <div className={["probeResult", `probeResult--${probeResult.verdict}`].join(" ")}>
              {(probeResult.verdict === "mismatch" || probeResult.conflict) && probeResult.reported_model && (
                <div className="probeResult__alert">
                  <AlertTriangle size={15} />
                  <span>响应模型：<strong>{probeResult.reported_model}</strong></span>
                </div>
              )}
              <div className="probeResult__rows">
                <div className="probeResult__row"><span>请求模型</span><code>{probeResult.requested_model}</code></div>
                <div className="probeResult__row"><span>上游请求模型</span><code>{probeResult.requested_model}</code></div>
                <div className="probeResult__row">
                  <span>响应模型</span>
                  <code className={probeResult.verdict === "mismatch" ? "is-danger" : ""}>{probeResult.reported_model ?? "—"}</code>
                </div>
              </div>
              <div className="probeResult__flow">
                <span className={["probeBadge", `probeBadge--${probeResult.verdict}`].join(" ")}>{verdictLabel(probeResult.verdict)}</span>
                <span className="probeResult__endpoint" title="探测端点（已脱敏）">{probeResult.endpoint}</span>
                {probeResult.http_status != null && <span className="probeResult__endpoint">HTTP {probeResult.http_status}</span>}
                {probeResult.observed.length > 1 && <span className="probeResult__endpoint">自报: {probeResult.observed.join(" / ")}</span>}
              </div>
              <p className="probeResult__detail">{probeResult.detail}</p>
            </div>
          )}
        </section>

        <section className="audit" aria-label="模型审计日报">
          <div className="audit__head">
            <div className="audit__headText">
              <h2><Layers size={16} /> 模型审计日报</h2>
              <p>读一天的日志：总请求、异常请求（路由/替换、Token 矛盾、无效模型、疑似降智），以及异常请求实际是什么模型。按北京时间。</p>
            </div>
            <div className="audit__controls">
              <input type="date" className="audit__date" value={reportDate} max={todayShanghai()} aria-label="选择日期" onChange={(e) => setReportDate(e.target.value)} />
              <button className="btn btn--soft btn--sm" disabled={reportLoading} aria-label="刷新日报" onClick={() => loadReport(reportDate)}>
                {reportLoading ? <Loader2 size={14} className="spin" /> : <RefreshCw size={14} />}
                刷新
              </button>
            </div>
          </div>
          {report && (
            <div className="audit__body">
              <div className="audit__tiles">
                <div className="auditTile"><span className="auditTile__label">分析请求</span><strong>{report.analyzed_requests.toLocaleString("zh-CN")}</strong><span className="auditTile__foot">HTTP 异常 {report.errors}</span></div>
                <div className="auditTile auditTile--alert"><span className="auditTile__label">异常请求</span><strong>{report.anomaly_total.toLocaleString("zh-CN")}</strong><span className="auditTile__foot">命中任一异常规则</span></div>
                <div className="auditTile auditTile--ok"><span className="auditTile__label">正常请求</span><strong>{report.clean_requests.toLocaleString("zh-CN")}</strong><span className="auditTile__foot">未命中异常</span></div>
              </div>
              <div className="audit__cats">
                {report.anomalies.map((cat) => (
                  <div key={cat.key} className={["auditCat", cat.count ? "is-hit" : ""].join(" ")}>
                    <span className="auditCat__count">{cat.count.toLocaleString("zh-CN")}</span>
                    <span className="auditCat__label">{cat.label}</span>
                  </div>
                ))}
              </div>
              <div className="audit__subhead"><ShieldAlert size={14} /> 异常请求实际是什么模型<span className="audit__hint">重中之重 · 每行标注证据级别</span></div>
              <div className="auditTableWrap">
                <table className="auditTable">
                  <thead><tr><th>请求模型</th><th>实际模型 · 家族或自报</th><th>证据级别</th><th className="num">次数</th></tr></thead>
                  <tbody>
                    {report.actual_model_breakdown.map((row, i) => (
                      <tr key={i}>
                        <td><code>{row.requested_model}</code></td>
                        <td><code className={row.evidence_level !== "undetermined" ? "is-danger" : ""}>{row.actual_model}</code></td>
                        <td><span className={["evTag", `evTag--${row.evidence_level}`].join(" ")}>{EVIDENCE_LABEL[row.evidence_level]}</span></td>
                        <td className="num">{row.count.toLocaleString("zh-CN")}</td>
                      </tr>
                    ))}
                    {!report.actual_model_breakdown.length && (
                      <tr><td className="auditEmpty" colSpan={4}>当日没有检测到异常请求。</td></tr>
                    )}
                  </tbody>
                </table>
              </div>
              {report.rows.length > 0 && (
                <details className="auditDetails">
                  <summary>展开异常请求明细（{report.rows.length} 条）</summary>
                  <div className="auditTableWrap">
                    <table className="auditTable">
                      <thead><tr><th>时间</th><th>来源</th><th>供应商</th><th>请求</th><th>实际</th><th className="num">输入</th><th className="num">输出</th><th>类型</th></tr></thead>
                      <tbody>
                        {report.rows.map((row) => (
                          <tr key={row.id}>
                            <td>{row.time}</td>
                            <td>{row.source === "ccswitch" ? "CC Switch" : "Codex"}</td>
                            <td>{row.provider}</td>
                            <td><code>{row.requested_model}</code></td>
                            <td><code className={row.evidence_level !== "undetermined" ? "is-danger" : ""}>{row.actual_model}</code></td>
                            <td className="num">{row.input_tokens ?? "—"}</td>
                            <td className="num">{row.output_tokens ?? "—"}</td>
                            <td>{row.categories.join("、")}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </details>
              )}
              <div className="audit__sources">
                {report.sources.map((src) => (
                  <span key={src.id} className={["auditSource", src.available ? "" : "is-off"].join(" ")}>{src.label}：{src.available ? `${src.records} 条` : "不可用"}</span>
                ))}
              </div>
              <ul className="audit__limits">
                {report.limitations.map((line, i) => <li key={i}>{line}</li>)}
              </ul>
            </div>
          )}
        </section>

        <section className="feed">
          <div className="feed__head">
            <div className="feed__headText">
              <h2>执行记录</h2>
              <p>命令、文件读取 / 新建 / 修改 / 删除、网络请求会按时间汇总在这里，当前显示 {filteredActivity.length} 条。</p>
            </div>
            <div className="feed__tools">
              <div className="searchBox">
                <Search size={15} />
                <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索命令 / 路径…" aria-label="搜索执行记录" />
                {search && (
                  <button className="searchBox__clear" aria-label="清除搜索" onClick={() => setSearch("")}>
                    <X size={13} />
                  </button>
                )}
              </div>
              <button className="btn btn--ghost btn--sm" disabled={!state.activity.length} onClick={clearActivity}>
                <Trash2 size={14} /> 清空
              </button>
            </div>
          </div>
          <ActivityTimeline events={filteredActivity} loading={loading} filtered={activityFilter !== "all" || !!search.trim()} />
        </section>
      </section>

      {settingsOpen && (
        <>
          <div className="overlay" onClick={closeSettings} aria-hidden="true" />
          <aside className="drawer" role="dialog" aria-modal="true" aria-labelledby="settings-title">
            <div className="drawer__head">
              <div>
                <h2 id="settings-title">设置</h2>
                <p>Codex 保安 · v{state.app.version}</p>
              </div>
              <button ref={drawerCloseRef} className="btn btn--icon" aria-label="关闭设置" onClick={closeSettings}>
                <X size={18} />
              </button>
            </div>

            <div className="field">
              <span className="railLabel">外观主题</span>
              <div className="segmented" role="group" aria-label="主题">
                <button className={theme === "light" ? "is-active" : ""} aria-pressed={theme === "light"} onClick={() => setTheme("light")}>
                  <Sun size={15} /> 浅色
                </button>
                <button className={theme === "dark" ? "is-active" : ""} aria-pressed={theme === "dark"} onClick={() => setTheme("dark")}>
                  <Moon size={15} /> 深色
                </button>
                <button className={theme === "system" ? "is-active" : ""} aria-pressed={theme === "system"} onClick={() => setTheme("system")}>
                  <Monitor size={15} /> 跟随系统
                </button>
              </div>
            </div>

            <div className="field">
              <span className="railLabel">运行设置</span>
              <div className="settingGroup">
                <Toggle checked={settings.background_run} onChange={(v) => updateSetting({ background_run: v })} icon={<Minimize2 size={16} />} label="后台运行" hint="关闭窗口时最小化到托盘，监控继续运行" />
                <Toggle checked={settings.autostart} onChange={(v) => updateSetting({ autostart: v })} icon={<Power size={16} />} label="开机自启动" hint={autostartHint} disabled={autostartDisabled} />
                <Toggle checked={settings.silent_start} onChange={(v) => updateSetting({ silent_start: v })} icon={<EyeOff size={16} />} label="开机静默启动" hint="开机自启时不弹窗，直接在后台监控（需先开启自启）" />
              </div>
            </div>

            <section className="updateCard">
              <div className="updateCard__row">
                <div className="updateCard__info">
                  <span className="railLabel">版本更新</span>
                  <strong>当前 v{state.app.version}</strong>
                  <small>
                    {update.status === "checking" && "正在检查最新版本…"}
                    {update.status === "latest" && (update.message || `已是最新版本${update.latest ? ` (v${update.latest})` : ""}`)}
                    {update.status === "available" && (update.message || `发现新版本 v${update.latest}`)}
                    {update.status === "downloading" && (update.message || `正在下载并安装 ${update.progress ?? 0}%，完成后将自动重启…`)}
                    {update.status === "error" && (update.message || "检查失败，可打开下载页手动安装")}
                    {update.status === "idle" &&
                      (state.app.updater_configured
                        ? "应用启动后会自动检查一次，也可手动检查新版本"
                        : state.app.portable_mode
                          ? "便携版使用下载页手动更新"
                          : "开发模式使用 GitHub Releases 下载页")}
                  </small>
                </div>
                {update.status === "available" ? (
                  <button className="btn btn--primary btn--sm" onClick={installUpdate}>
                    <ArrowUpCircle size={15} /> 立即更新 v{update.latest}
                  </button>
                ) : update.status === "downloading" ? (
                  <button className="btn btn--primary btn--sm" disabled>
                    <Download size={14} className="spin" /> 下载中 {update.progress ?? 0}%
                  </button>
                ) : (
                  <div className="updateCard__actions">
                    <button className="btn btn--soft btn--sm" disabled={update.status === "checking"} onClick={checkUpdate}>
                      {update.status === "checking" ? <Loader2 size={14} className="spin" /> : update.status === "latest" ? <Check size={14} /> : <RefreshCw size={14} />}
                      {update.status === "checking" ? "检查中" : update.status === "latest" ? "已最新" : "检查更新"}
                    </button>
                    <button className="btn btn--soft btn--sm" onClick={() => runManaged("update", "已打开下载页", () => invoke("open_releases"))}>
                      <ExternalLink size={14} /> 下载页
                    </button>
                  </div>
                )}
              </div>
            </section>

            <section className="installCard">
              <div className="installCard__head">
                <span className="railLabel">安装管理</span>
                <strong>{state.app.portable_mode ? "便携版" : state.app.bundle_managed ? "安装版" : "开发运行"}</strong>
                <small title={state.app.install_dir}>{state.app.install_dir || "未检测到安装目录"}</small>
              </div>
              <div className="installCard__actions">
                <button className="btn btn--soft btn--sm" onClick={() => runManaged("install-dir", "已打开安装目录", () => invoke("open_install_dir"))} disabled={managementBusy !== null}>
                  {managementBusy === "install-dir" ? <Loader2 size={14} className="spin" /> : <FolderOpen size={14} />} 安装目录
                </button>
                <button className="btn btn--soft btn--sm" onClick={() => runManaged("uninstall", "已打开系统卸载设置", () => invoke("open_uninstall_settings"))} disabled={managementBusy !== null}>
                  <Trash2 size={14} /> 卸载
                </button>
              </div>
            </section>

            <button className="logDirRow" onClick={() => runManaged("log-dir", "已打开监控日志目录", () => invoke("open_log_dir"))} disabled={managementBusy !== null}>
              <span className="logDirRow__icon" aria-hidden="true">
                {managementBusy === "log-dir" ? <Loader2 size={16} className="spin" /> : <FolderOpen size={16} />}
              </span>
              <span className="logDirRow__text">
                <strong>监控日志目录</strong>
                <small>打开 Codex 会话日志所在文件夹（监控数据来源）</small>
              </span>
            </button>

            <p className="drawer__note">{state.app.sessions_dir}</p>
          </aside>
        </>
      )}

      <div className={["toast", toast ? "is-show" : ""].join(" ")} role="status" aria-live="polite" aria-atomic="true">
        <ShieldCheck size={16} />
        {toast}
      </div>
    </main>
  );
}

function UpstreamBar({
  loading,
  upstream,
  sources,
  running,
  fallback
}: {
  loading: boolean;
  upstream?: DiscoveredProvider;
  sources: AppState["discovery"]["sources"];
  running: boolean;
  fallback: string;
}) {
  if (loading) {
    return (
      <div className="upstreamBar upstreamBar--plain">
        <span className="pulse" aria-hidden="true" />
        <div className="upstreamBar__info">
          <small>正在初始化</small>
          <strong>正在读取本机配置…</strong>
        </div>
      </div>
    );
  }
  if (!upstream) {
    return (
      <div className="upstreamBar upstreamBar--plain">
        <span className="upstreamBar__logo muted" aria-hidden="true">
          <Shield size={20} />
        </span>
        <div className="upstreamBar__info">
          <small>未检测到上游</small>
          <strong>{running ? "本机会话审计运行中" : "本机会话审计未启动"}</strong>
          <p className="upstreamBar__hint">{fallback}</p>
        </div>
      </div>
    );
  }
  return (
    <div className="upstreamBar">
      <span className="upstreamBar__logo" aria-hidden="true">
        <SourceLogo source={upstream.source} size={28} />
      </span>
      <div className="upstreamBar__info">
        <div className="upstreamBar__title">
          <span className="pulse-inline">
            <span className={["pulse", running ? "is-online" : ""].join(" ")} role="img" aria-label={running ? "监控运行中" : "监控未启用"} />
          </span>
          <strong>{upstream.name}</strong>
          <span className={["badge", running ? "badge--live" : "badge--idle"].join(" ")}>{running ? "监控中" : "未监控"}</span>
          <span className={["tag", upstream.status === "ready" ? "tag--ok" : "tag--muted"].join(" ")}>{providerStatus(upstream.status)}</span>
        </div>
        <div className="upstreamBar__meta">
          <code className="upstreamBar__url" title={upstream.source_path}>
            {upstream.base_url || "登录态 / 本地配置"}
          </code>
          <span className="sep" aria-hidden="true" />
          <span>{upstream.model || upstream.protocol || "Codex"}</span>
          <span className="sep" aria-hidden="true" />
          <span title={upstream.notes.join("\n")}>{upstream.has_api_key ? upstream.masked_api_key || "已配置 Key" : upstream.status_text}</span>
        </div>
        {!!sources.length && (
          <div className="upstreamBar__sources">
            <span className="srcLabel">配置来源</span>
            {sources.filter((source) => source.exists).map((source) => (
              <span key={source.id} className="srcChip" title={source.status === "error" ? source.message : source.path}>
                <SourceLogo source={source.id} size={14} />
                {source.label}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function Kpi({ tone, icon, label, value, detail, active, onClick }: { tone: KpiTone; icon: React.ReactNode; label: string; value: React.ReactNode; detail: React.ReactNode; active?: boolean; onClick?: () => void }) {
  return (
    <button className={["kpi", `kpi--${tone}`, active ? "is-active" : ""].join(" ")} onClick={onClick} aria-pressed={active}>
      <span className="kpi__icon" aria-hidden="true">{icon}</span>
      <span className="kpi__body">
        <small className="kpi__label">{label}</small>
        <strong className="kpi__value">{value}</strong>
        <small className="kpi__detail">{detail}</small>
      </span>
    </button>
  );
}

function Toggle({ checked, onChange, icon, label, hint, disabled = false }: { checked: boolean; onChange: (value: boolean) => void; icon: React.ReactNode; label: string; hint: string; disabled?: boolean }) {
  return (
    <button className={["settingRow", checked ? "is-on" : "", disabled ? "is-disabled" : ""].join(" ")} role="switch" aria-checked={checked} onClick={() => onChange(!checked)} disabled={disabled}>
      <span className="settingRow__icon" aria-hidden="true">{icon}</span>
      <span className="settingRow__text">
        <strong>{label}</strong>
        <small>{hint}</small>
      </span>
      <span className="toggle" aria-hidden="true"><span className="toggle__knob" /></span>
    </button>
  );
}

function ActivityTimeline({ events, loading, filtered }: { events: ActivityEvent[]; loading: boolean; filtered: boolean }) {
  const ordered = useMemo(() => [...events].reverse(), [events]);
  if (loading) {
    return (
      <div className="feed__loading" aria-label="正在扫描">
        <Loader2 size={20} className="spin" />
        <span>正在扫描本机配置…</span>
      </div>
    );
  }
  if (!events.length) {
    return (
      <div className="feed__empty">
        <span className="feed__emptyIcon" aria-hidden="true">
          <Shield size={26} />
        </span>
        <h3>{filtered ? "没有匹配的记录" : "还没有监控事件"}</h3>
        <p>{filtered ? "换一个筛选条件或搜索词，或等待新的活动进入。" : "Codex 的命令、文件读写 / 删除 / 修改与网络请求会按时间汇总在这里。"}</p>
      </div>
    );
  }
  return (
    <div className="timeline" aria-label="活动时间线">
      {ordered.map((event) => (
        <article className={["event", `is-${severityTone(event.severity)}`].join(" ")} key={event.id}>
          <span className={["event__icon", `kind-${event.kind}`].join(" ")} aria-hidden="true">
            <ActivityKindIcon kind={event.kind} size={16} />
          </span>
          <div className="event__body">
            <div className="event__title">
              <h3>{event.title}</h3>
              <span className={["sev", `sev--${severityTone(event.severity)}`].join(" ")}>{severityLabel(event.severity)}</span>
              {event.source && <small className="event__source">{event.source}</small>}
              <time dateTime={event.timestamp}>{formatTime(event.timestamp)}</time>
            </div>
            <p>{event.summary}</p>
            {event.command && <code className="event__cmd">{event.command}</code>}
            {!!event.paths.length && (
              <div className="chips">
                {event.paths.slice(0, 4).map((path) => (
                  <code key={path} className="chip">{path}</code>
                ))}
                {event.paths.length > 4 && <span className="chip chip--more">+{event.paths.length - 4} 个路径</span>}
              </div>
            )}
          </div>
        </article>
      ))}
    </div>
  );
}

function providerStatus(status: string) {
  return ({ ready: "已配置", "needs-auth": "缺少凭据", "auth-unverified": "认证待确认", unconfigured: "未配置" } as Record<string, string>)[status] || status;
}

function isRisk(event: ActivityEvent) {
  return event.kind === "risk" || event.severity === "high" || event.severity === "critical";
}

function filterCount(filter: ActivityFilter, events: ActivityEvent[]) {
  if (filter === "all") return events.length;
  if (filter === "risk") return events.filter(isRisk).length;
  return events.filter((event) => event.kind === filter).length;
}

function severityTone(severity: string) {
  return ({ info: "info", low: "low", medium: "medium", high: "high", critical: "critical" } as Record<string, string>)[severity] || "info";
}

function severityLabel(severity: string) {
  return ({ info: "信息", low: "低", medium: "中", high: "高", critical: "严重" } as Record<string, string>)[severity] || severity;
}

function formatTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value || "--:--";
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function SourceLogo({ source, size = 18 }: { source: string; size?: number }) {
  if (source === "ccswitch") {
    return <img className="srcImg" src={ccswitchIcon} width={size} height={size} alt="" draggable={false} />;
  }
  if (source === "codexplusplus") return <CodexMark size={size} plus />;
  if (source === "codex-config") return <CodexMark size={size} />;
  return <Shield size={size} strokeWidth={2.2} />;
}

// OpenAI / Codex 官方标志（花结）
function CodexMark({ size = 18, plus = false }: { size?: number; plus?: boolean }) {
  return (
    <span className="codexMark" style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M22.282 9.821a5.985 5.985 0 0 0-.516-4.91 6.046 6.046 0 0 0-6.51-2.9A6.065 6.065 0 0 0 4.981 4.182a5.985 5.985 0 0 0-3.998 2.9 6.046 6.046 0 0 0 .743 7.097 5.98 5.98 0 0 0 .51 4.911 6.051 6.051 0 0 0 6.515 2.9A5.985 5.985 0 0 0 13.26 24a6.056 6.056 0 0 0 5.772-4.206 5.99 5.99 0 0 0 3.997-2.9 6.056 6.056 0 0 0-.747-7.073zM13.26 22.43a4.476 4.476 0 0 1-2.876-1.04l.141-.081 4.779-2.758a.795.795 0 0 0 .392-.681v-6.737l2.02 1.168a.071.071 0 0 1 .038.052v5.583a4.504 4.504 0 0 1-4.494 4.494zM3.6 18.305a4.471 4.471 0 0 1-.535-3.014l.142.085 4.783 2.759a.771.771 0 0 0 .78 0l5.843-3.369v2.332a.08.08 0 0 1-.033.062L9.74 19.95a4.5 4.5 0 0 1-6.14-1.646zM2.34 7.896a4.485 4.485 0 0 1 2.366-1.973V11.6a.766.766 0 0 0 .388.676l5.815 3.355-2.02 1.168a.076.076 0 0 1-.071 0l-4.83-2.786A4.504 4.504 0 0 1 2.34 7.872zm16.597 3.856-5.833-3.387L15.119 7.2a.076.076 0 0 1 .071 0l4.83 2.791a4.494 4.494 0 0 1-.676 8.105v-5.678a.79.79 0 0 0-.407-.667zm2.01-3.023-.141-.085-4.774-2.782a.776.776 0 0 0-.785 0L9.409 9.23V6.897a.066.066 0 0 1 .028-.061l4.83-2.787a4.5 4.5 0 0 1 6.68 4.66zM8.307 12.863l-2.02-1.164a.08.08 0 0 1-.038-.057V6.075a4.5 4.5 0 0 1 7.376-3.453l-.142.08L8.704 5.46a.795.795 0 0 0-.393.681zm1.098-2.365 2.602-1.5 2.607 1.5v2.999l-2.597 1.5-2.607-1.5z" />
      </svg>
      {plus && <span className="codexMark__plus" aria-hidden="true">+</span>}
    </span>
  );
}

function ActivityKindIcon({ kind, size = 16 }: { kind: ActivityKind | "all"; size?: number }) {
  if (kind === "all") return <Layers size={size} strokeWidth={2.2} />;
  if (kind === "file-read") return <Eye size={size} strokeWidth={2.2} />;
  if (kind === "file-create") return <FilePlus size={size} strokeWidth={2.2} />;
  if (kind === "file-delete") return <FileMinus size={size} strokeWidth={2.2} />;
  if (kind === "file-modify") return <FilePen size={size} strokeWidth={2.2} />;
  if (kind === "network") return <Globe size={size} strokeWidth={2.2} />;
  if (kind === "risk") return <AlertTriangle size={size} strokeWidth={2.2} />;
  return <Terminal size={size} strokeWidth={2.2} />;
}
