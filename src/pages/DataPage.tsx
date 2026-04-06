import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { useUiStore } from "../store/uiStore";

interface Row {
  day: string;
  plannedCount: number;
  doneCount: number;
  completionRate: number;
  focusMinutes: number;
  busyMinutes: number;
  firstTaskAt: string | null;
  lastCompletionAt: string | null;
}

function pct(n: number) {
  if (!Number.isFinite(n)) return "—";
  return `${(n * 100).toFixed(1)}%`;
}

export function DataPage() {
  const tab = useUiStore((s) => s.tab);
  const [rows, setRows] = useState<Row[]>([]);
  const [pending, setPending] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const load = useCallback(() => {
    setPending(true);
    setErr(null);
    void invoke<Row[]>("list_analytics_snapshot", { limit: 180 })
      .then(setRows)
      .catch((e) => {
        setErr(String(e));
        setRows([]);
      })
      .finally(() => setPending(false));
  }, []);

  useEffect(() => {
    if (tab === "data") load();
  }, [tab, load]);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-zinc-200">日聚合快照</h2>
          <p className="mt-1 max-w-xl text-xs text-zinc-500">
            数据来自本地 SQLite 视图{" "}
            <code className="rounded bg-zinc-800 px-1 py-0.5 text-[10px]">
              v_analytics_snapshot
            </code>
            （基于 <code className="rounded bg-zinc-800 px-1 py-0.5 text-[10px]">daily_rollups</code>
            ）。切换到此页会自动刷新。
          </p>
        </div>
        <button
          type="button"
          disabled={pending}
          onClick={load}
          className="rounded-lg border border-zinc-600 px-3 py-1.5 text-sm text-zinc-300 hover:bg-zinc-800 disabled:opacity-50"
        >
          {pending ? "加载中…" : "刷新"}
        </button>
      </div>

      {err && (
        <p className="rounded-lg border border-rose-900/60 bg-rose-950/40 px-3 py-2 text-sm text-rose-200">
          {err}
        </p>
      )}

      <div className="min-h-0 flex-1 overflow-auto rounded-xl border border-zinc-800">
        <table className="w-full min-w-[880px] border-collapse text-left text-xs">
          <thead className="sticky top-0 z-10 border-b border-zinc-800 bg-zinc-950/95 backdrop-blur">
            <tr className="text-zinc-500">
              <th className="px-3 py-2 font-medium">日期</th>
              <th className="px-3 py-2 font-medium">计划数</th>
              <th className="px-3 py-2 font-medium">完成数</th>
              <th className="px-3 py-2 font-medium">完成率</th>
              <th className="px-3 py-2 font-medium">计划分钟</th>
              <th className="px-3 py-2 font-medium">忙时分钟</th>
              <th className="px-3 py-2 font-medium">首任务</th>
              <th className="px-3 py-2 font-medium">末完成</th>
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 && !pending ? (
              <tr>
                <td
                  colSpan={8}
                  className="px-3 py-8 text-center text-zinc-500"
                >
                  暂无 rollup 数据。完成或排期任务后会出现对应日期行。
                </td>
              </tr>
            ) : (
              rows.map((r) => (
                <tr
                  key={r.day}
                  className="border-b border-zinc-800/80 hover:bg-zinc-900/40"
                >
                  <td className="whitespace-nowrap px-3 py-2 font-mono text-zinc-200">
                    {r.day}
                  </td>
                  <td className="px-3 py-2 text-zinc-300">{r.plannedCount}</td>
                  <td className="px-3 py-2 text-zinc-300">{r.doneCount}</td>
                  <td className="px-3 py-2 text-zinc-300">
                    {pct(r.completionRate)}
                  </td>
                  <td className="px-3 py-2 text-zinc-300">{r.focusMinutes}</td>
                  <td className="px-3 py-2 text-zinc-300">{r.busyMinutes}</td>
                  <td className="max-w-[140px] truncate px-3 py-2 font-mono text-[10px] text-zinc-500">
                    {r.firstTaskAt ?? "—"}
                  </td>
                  <td className="max-w-[140px] truncate px-3 py-2 font-mono text-[10px] text-zinc-500">
                    {r.lastCompletionAt ?? "—"}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
