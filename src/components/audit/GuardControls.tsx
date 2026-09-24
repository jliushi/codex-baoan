import { ShieldCheck, ShieldAlert, PlugZap, Unplug, Loader2 } from "lucide-react";
import type { GuardStatus } from "../../types/audit";

interface GuardControlsProps {
  status: GuardStatus | null;
  busy: boolean;
  notice: string | null;
  onInstallCert: () => void;
  onAttach: () => void;
  onDetach: () => void;
}

// 接入状态栏：一键信任证书 + 一键接入/断开 CC Switch（对标 CC Switch 的操作入口）。
export function GuardControls({
  status,
  busy,
  notice,
  onInstallCert,
  onAttach,
  onDetach,
}: GuardControlsProps) {
  const certOk = status?.cert_trusted ?? false;
  const ccFound = status?.ccswitch_found ?? false;
  const attached = status?.attached ?? false;

  return (
    <section className="guard-bar" aria-label="接入控制">
      <div className="guard-step">
        <span className={`guard-dot ${certOk ? "ok" : "warn"}`} aria-hidden="true" />
        <div className="guard-copy">
          <strong>本机证书</strong>
          <small>{certOk ? "已信任，可解密本机流量做指纹" : "未信任，需装入当前用户信任库"}</small>
        </div>
        {!certOk && (
          <button className="guard-btn" type="button" disabled={busy} onClick={onInstallCert}>
            {busy ? <Loader2 size={14} className="spin" /> : <ShieldCheck size={14} />}一键信任证书
          </button>
        )}
        {certOk && <ShieldCheck size={16} className="guard-ok-icon" aria-hidden="true" />}
      </div>

      <div className="guard-step">
        <span className={`guard-dot ${attached ? "ok" : ccFound ? "warn" : "idle"}`} aria-hidden="true" />
        <div className="guard-copy">
          <strong>接入 CC Switch</strong>
          <small>
            {!ccFound
              ? "未找到 CC Switch"
              : attached
                ? `已接入（出口经 ${status?.proxy_url}），重启一次 CC Switch 生效`
                : "未接入；接入后新会话流量将被观察"}
          </small>
        </div>
        {ccFound && !attached && (
          <button className="guard-btn" type="button" disabled={busy} onClick={onAttach}>
            {busy ? <Loader2 size={14} className="spin" /> : <PlugZap size={14} />}一键接入
          </button>
        )}
        {ccFound && attached && (
          <button className="guard-btn ghost" type="button" disabled={busy} onClick={onDetach}>
            <Unplug size={14} />断开
          </button>
        )}
        {!certOk && <ShieldAlert size={15} className="guard-warn-icon" aria-hidden="true" />}
      </div>

      {notice && <p className="guard-notice">{notice}</p>}
    </section>
  );
}
