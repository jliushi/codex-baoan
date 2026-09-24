import { ShieldCheck, ShieldAlert, Loader2, Info } from "lucide-react";
import type { GuardStatus } from "../../types/audit";

interface GuardControlsProps {
  status: GuardStatus | null;
  busy: boolean;
  notice: string | null;
  onInstallCert: () => void;
}

// 接入状态栏：一键信任证书；CC Switch 出口是否已指向本工具（只读检测，本工具不改动 CC Switch）。
export function GuardControls({ status, busy, notice, onInstallCert }: GuardControlsProps) {
  const certOk = status?.cert_trusted ?? false;
  const ccFound = status?.ccswitch_found ?? false;
  const routed = status?.routed ?? false;
  const proxyUrl = status?.proxy_url ?? "http://127.0.0.1:8899";

  return (
    <section className="guard-bar" aria-label="接入控制">
      <div className="guard-step">
        <span className={`guard-dot ${certOk ? "ok" : "warn"}`} aria-hidden="true" />
        <div className="guard-copy">
          <strong>本机证书</strong>
          <small>{certOk ? "已信任，可解密本机流量做指纹" : "未信任，需装入当前用户信任库"}</small>
        </div>
        {certOk ? (
          <ShieldCheck size={16} className="guard-ok-icon" aria-hidden="true" />
        ) : (
          <button className="guard-btn" type="button" disabled={busy} onClick={onInstallCert}>
            {busy ? <Loader2 size={14} className="spin" /> : <ShieldCheck size={14} />}一键信任证书
          </button>
        )}
      </div>

      <div className="guard-step">
        <span className={`guard-dot ${routed ? "ok" : "idle"}`} aria-hidden="true" />
        <div className="guard-copy">
          <strong>流量接入</strong>
          <small>
            {!ccFound
              ? "未找到 CC Switch"
              : routed
                ? "CC Switch 出口已指向本工具，正在观察"
                : `未观察到流量。把你代理层的出口指向 ${proxyUrl} 即可（本工具不改动 CC Switch）`}
          </small>
        </div>
        {routed ? (
          <ShieldCheck size={15} className="guard-ok-icon" aria-hidden="true" />
        ) : (
          <Info size={15} className="guard-warn-icon" aria-hidden="true" />
        )}
      </div>

      {notice && <p className="guard-notice">{notice}</p>}
      {!certOk && ccFound && (
        <p className="guard-notice">
          <ShieldAlert size={13} /> 指纹需要先信任证书才能解密本机流量。
        </p>
      )}
    </section>
  );
}
