import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { GroupVerdict, GuardStatus } from "../types/audit";

// 接入状态 + 分词器指纹：读后端 MITM 相关命令。证书/接入按钮触发后刷新状态。
export function useGuard(date: string) {
  const [status, setStatus] = useState<GuardStatus | null>(null);
  const [fingerprints, setFingerprints] = useState<GroupVerdict[]>([]);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await invoke<GuardStatus>("guard_status"));
    } catch {
      setStatus(null);
    }
  }, []);

  const refreshFingerprints = useCallback(async () => {
    try {
      setFingerprints(await invoke<GroupVerdict[]>("fingerprint_report", { date }));
    } catch {
      setFingerprints([]);
    }
  }, [date]);

  useEffect(() => {
    void refreshStatus();
    void refreshFingerprints();
  }, [refreshStatus, refreshFingerprints]);

  const run = useCallback(
    async (cmd: "install_cert" | "uninstall_cert") => {
      setBusy(true);
      setNotice(null);
      try {
        await invoke<unknown>(cmd);
      } catch (cause) {
        setNotice(typeof cause === "string" ? cause : "操作失败");
      } finally {
        setBusy(false);
        await refreshStatus();
      }
    },
    [refreshStatus],
  );

  return { status, fingerprints, busy, notice, refreshStatus, refreshFingerprints, run };
}
