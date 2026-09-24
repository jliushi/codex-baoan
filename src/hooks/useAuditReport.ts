import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AuditReport } from "../types/audit";

function getErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "读取审计数据失败，请重试。";
}

export function useAuditReport(date: string) {
  const [report, setReport] = useState<AuditReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const requestId = useRef(0);

  const refresh = useCallback(async () => {
    const id = ++requestId.current;
    setLoading(true);
    setError(null);

    try {
      const nextReport = await invoke<AuditReport>("audit_report", { date });
      if (requestId.current === id) setReport(nextReport);
    } catch (cause) {
      if (requestId.current === id) setError(getErrorMessage(cause));
    } finally {
      if (requestId.current === id) setLoading(false);
    }
  }, [date]);

  useEffect(() => {
    void refresh();
    return () => {
      requestId.current += 1;
    };
  }, [refresh]);

  return { report, loading, error, refresh };
}
