import { useVirtualizer } from "@tanstack/react-virtual";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";

interface Row {
  id: string;
  kind: string;
  periodKey: string;
  modelText: string;
  createdAt: string;
  triggerKind: string;
}

export function SummariesPage() {
  const [rows, setRows] = useState<Row[]>([]);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [exportBusy, setExportBusy] = useState(false);
  const parentRef = useRef<HTMLDivElement>(null);

  const reload = () => {
    void invoke<Row[]>("list_summaries", { limit: 80 })
      .then(setRows)
      .catch(() => setRows([]));
  };

  useEffect(() => {
    reload();
  }, []);

  const v = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 220,
    overscan: 3,
  });

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={exportBusy}
          className="rounded-lg border border-zinc-600 px-3 py-1.5 text-sm text-zinc-300 hover:bg-zinc-800 disabled:opacity-50"
          onClick={() => {
            setExportBusy(true);
            setExportPath(null);
            void invoke<string>("export_summaries_markdown")
              .then((p) => {
                setExportPath(p);
                reload();
              })
              .catch(() => setExportPath("导出失败"))
              .finally(() => setExportBusy(false));
          }}
        >
          {exportBusy ? "导出中…" : "导出为 Markdown"}
        </button>
        {exportPath && (
          <span className="text-xs text-zinc-500 break-all">已写入：{exportPath}</span>
        )}
      </div>
    <div
      ref={parentRef}
      className="min-h-0 flex-1 overflow-y-auto rounded-xl border border-zinc-800"
    >
      <div className="relative" style={{ height: v.getTotalSize() }}>
        {v.getVirtualItems().map((vi) => {
          const r = rows[vi.index];
          return (
            <article
              key={r.id}
              className="absolute left-0 top-0 w-full border-b border-zinc-800 p-4 cv-auto"
              style={{ transform: `translateY(${vi.start}px)` }}
            >
              <div className="mb-2 flex flex-wrap gap-2 text-xs text-zinc-500">
                <span className="rounded bg-zinc-800 px-2 py-0.5">{r.kind}</span>
                <span>{r.periodKey}</span>
                <span>{r.triggerKind}</span>
                <span>{r.createdAt}</span>
              </div>
              <div className="prose prose-invert prose-sm max-w-none prose-p:leading-relaxed">
                <ReactMarkdown>{r.modelText}</ReactMarkdown>
              </div>
            </article>
          );
        })}
      </div>
    </div>
    </div>
  );
}
