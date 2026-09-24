import { Fingerprint } from "lucide-react";
import type { GroupVerdict } from "../../types/audit";

interface FingerprintPanelProps {
  verdicts: GroupVerdict[];
}

function familyTone(family: string): string {
  if (family === "o200k" || family === "cl100k") return "green";
  if (family === "suspected_non_gpt") return "red";
  return "muted";
}

// 分词器指纹：把 MITM 抓到的真实流量按请求模型分组，展示推断的模型家族。
export function FingerprintPanel({ verdicts }: FingerprintPanelProps) {
  return (
    <section className="panel fingerprint-panel">
      <div className="panel-heading">
        <div>
          <h2>分词器指纹</h2>
          <p>由本机 MITM 抓到的真实请求推断的模型家族（家族级，非权重认证）</p>
        </div>
        <span className="panel-icon" aria-hidden="true">
          <Fingerprint size={17} strokeWidth={1.8} />
        </span>
      </div>
      {verdicts.length === 0 ? (
        <p className="fingerprint-empty">
          暂无可指纹的样本。信任证书并接入 CC Switch、重启后跑几轮 Codex，这里会出现结果。
        </p>
      ) : (
        <div className="table-wrap">
          <table className="data-table">
            <thead>
              <tr>
                <th>请求模型</th>
                <th>上游自报</th>
                <th>推断家族</th>
                <th className="num">样本</th>
                <th className="num">slope</th>
                <th>说明</th>
              </tr>
            </thead>
            <tbody>
              {verdicts.map((v) => (
                <tr key={v.requested_model}>
                  <td><code>{v.requested_model}</code></td>
                  <td><code>{v.reported_models.join(", ") || "—"}</code></td>
                  <td>
                    <span className={`family-tag tone-${familyTone(v.family)}`}>{v.family_label}</span>
                  </td>
                  <td className="num">{v.samples}</td>
                  <td className="num">{v.slope ? v.slope.toFixed(3) : "—"}</td>
                  <td className="fingerprint-note">{v.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
