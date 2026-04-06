import { invoke, isTauri } from "@tauri-apps/api/core";
import { clsx } from "clsx";
import { useCallback, useEffect, useMemo, useState, useTransition } from "react";
import { DayTimeline, type BusyDto, type PlanDto } from "../components/DayTimeline";
import { useUiStore } from "../store/uiStore";

type DayOv = { day: string; planCount: number; busyCount: number };

/** 与后端 `CompleteResult`（camelCase）一致 */
type CompletePlanResult = {
  completedAt: string;
  wasOnTime: boolean;
  dailySummaryTriggered: boolean;
  taskTitle: string;
  feedbackText: string;
};

function MonthStrip({
  selectedDay,
  setSelectedDay,
}: {
  selectedDay: string;
  setSelectedDay: (d: string) => void;
}) {
  const calendarRevision = useUiStore((s) => s.calendarRevision);
  const [cursor, setCursor] = useState(() => {
    const [y, m] = selectedDay.split("-").map(Number);
    return { y, m };
  });
  const [cells, setCells] = useState<DayOv[]>([]);

  useEffect(() => {
    const [y, m] = selectedDay.split("-").map(Number);
    setCursor({ y, m });
  }, [selectedDay]);

  useEffect(() => {
    void invoke<DayOv[]>("calendar_month_overview", {
      year: cursor.y,
      month: cursor.m,
    })
      .then(setCells)
      .catch(() => setCells([]));
  }, [cursor.y, cursor.m, calendarRevision]);

  const map = useMemo(() => new Map(cells.map((c) => [c.day, c])), [cells]);
  const first = new Date(cursor.y, cursor.m - 1, 1);
  const pad = first.getDay();
  const lastDate = new Date(cursor.y, cursor.m, 0).getDate();

  const shift = (delta: number) => {
    setCursor((c) => {
      let nm = c.m + delta;
      let ny = c.y;
      while (nm > 12) {
        nm -= 12;
        ny += 1;
      }
      while (nm < 1) {
        nm += 12;
        ny -= 1;
      }
      return { y: ny, m: nm };
    });
  };

  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/40 p-4">
      <div className="mb-3 flex items-center justify-between gap-2">
        <button
          type="button"
          className="rounded border border-zinc-700 px-2 py-1 text-sm text-zinc-300 hover:bg-zinc-800"
          onClick={() => shift(-1)}
        >
          ‹
        </button>
        <span className="text-sm font-medium text-zinc-200">
          {cursor.y} 年 {cursor.m} 月
        </span>
        <button
          type="button"
          className="rounded border border-zinc-700 px-2 py-1 text-sm text-zinc-300 hover:bg-zinc-800"
          onClick={() => shift(1)}
        >
          ›
        </button>
      </div>
      <div className="grid grid-cols-7 gap-1 text-center text-[10px] text-zinc-500">
        {["日", "一", "二", "三", "四", "五", "六"].map((w) => (
          <span key={w}>{w}</span>
        ))}
      </div>
      <div className="mt-1 grid grid-cols-7 gap-1">
        {Array.from({ length: pad }, (_, i) => (
          <div key={`pad-${i}`} />
        ))}
        {Array.from({ length: lastDate }, (_, i) => {
          const d = i + 1;
          const dayStr = `${cursor.y}-${String(cursor.m).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
          const c = map.get(dayStr);
          return (
            <button
              key={dayStr}
              type="button"
              onClick={() => setSelectedDay(dayStr)}
              className={clsx(
                "min-h-[52px] rounded border p-1 text-left text-[11px] transition-colors",
                selectedDay === dayStr
                  ? "border-sky-500 bg-sky-950/40"
                  : "border-zinc-800 hover:bg-zinc-800/40",
              )}
            >
              <div className="font-medium text-zinc-200">{d}</div>
              <div className="text-[10px] text-zinc-500">
                {c && (c.planCount > 0 || c.busyCount > 0)
                  ? `计${c.planCount} 忙${c.busyCount}`
                  : "—"}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}

export function CalendarPage() {
  const selectedDay = useUiStore((s) => s.selectedDay);
  const setSelectedDay = useUiStore((s) => s.setSelectedDay);
  const calendarRevision = useUiStore((s) => s.calendarRevision);
  const bumpCalendar = useUiStore((s) => s.bumpCalendar);
  const [busy, setBusy] = useState<BusyDto[]>([]);
  const [plans, setPlans] = useState<PlanDto[]>([]);
  const [pending, startTransition] = useTransition();
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  /** 引擎返回的鼓励（不依赖事件通道，日历页内必显） */
  const [encouragement, setEncouragement] = useState<{
    taskTitle: string;
    text: string;
    wasOnTime: boolean;
  } | null>(null);

  const load = useCallback(() => {
    startTransition(async () => {
      try {
        const r = await invoke<[BusyDto[], PlanDto[]]>("calendar_list_day", {
          day: selectedDay,
        });
        setBusy(r[0]);
        setPlans(r[1]);
      } catch {
        setBusy([]);
        setPlans([]);
      }
    });
  }, [selectedDay]);

  useEffect(() => {
    load();
  }, [load, calendarRevision]);

  const onComplete = useCallback(
    (taskId: string) => {
      if (!isTauri()) {
        setActionError("请在桌面版应用内标记完成；浏览器预览无法调用本地引擎。");
        return;
      }
      setActionError(null);
      setEncouragement(null);
      setActionBusy(true);
      void invoke<CompletePlanResult>("complete_plan_item", { taskId })
        .then((r) => {
          setEncouragement({
            taskTitle: r.taskTitle,
            text: r.feedbackText,
            wasOnTime: r.wasOnTime,
          });
          bumpCalendar();
          load();
        })
        .catch((e) => setActionError(String(e)))
        .finally(() => setActionBusy(false));
    },
    [load, bumpCalendar],
  );

  const onSkip = useCallback(
    (taskId: string) => {
      if (!isTauri()) {
        setActionError("请在桌面版应用内操作；浏览器预览无法调用本地引擎。");
        return;
      }
      setActionError(null);
      setActionBusy(true);
      void invoke("skip_plan_item", { taskId })
        .then(() => {
          bumpCalendar();
          load();
        })
        .catch((e) => setActionError(String(e)))
        .finally(() => setActionBusy(false));
    },
    [load, bumpCalendar],
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 lg:flex-row lg:items-start lg:gap-6">
      <MonthStrip selectedDay={selectedDay} setSelectedDay={setSelectedDay} />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <label className="text-sm text-zinc-400">
          日期
          <input
            type="date"
            className="ml-2 rounded border border-zinc-700 bg-zinc-950 px-2 py-1"
            value={selectedDay}
            onChange={(e) => setSelectedDay(e.target.value)}
          />
        </label>
        {(pending || actionBusy) && (
          <span className="text-xs text-zinc-500">处理中…</span>
        )}
      </div>
      <p className="prose prose-invert prose-sm max-w-none text-zinc-400">
        红色为不可用块，绿色为计划；已跳过的任务不在此显示。可上下滚动查看全天。
      </p>
      {encouragement && (
        <div
          className="rounded-xl border border-sky-500/35 bg-sky-950/35 px-4 py-3 text-sm shadow-sm"
          role="status"
        >
          <div className="mb-1.5 flex items-center justify-between gap-2">
            <span className="text-[11px] font-semibold uppercase tracking-wide text-sky-200/90">
              完成鼓励
              <span
                className={clsx(
                  "ml-2 font-normal normal-case",
                  encouragement.wasOnTime ? "text-emerald-300/90" : "text-amber-300/90",
                )}
              >
                {encouragement.wasOnTime ? "· 在计划截止前完成" : "· 截止后完成"}
              </span>
            </span>
            <button
              type="button"
              className="rounded px-1.5 text-xs text-zinc-400 hover:bg-white/5 hover:text-zinc-200"
              onClick={() => setEncouragement(null)}
            >
              关闭
            </button>
          </div>
          <p className="leading-relaxed text-zinc-100">
            <span className="font-medium text-sky-100/95">
              {encouragement.taskTitle}
            </span>
            <span className="text-zinc-400"> · </span>
            {encouragement.text}
          </p>
        </div>
      )}
      {actionError && (
        <div
          role="alert"
          className="rounded-lg border border-rose-500/40 bg-rose-950/50 px-3 py-2 text-sm text-rose-100"
        >
          {actionError}
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden pr-1">
        <DayTimeline
          viewDay={selectedDay}
          busy={busy}
          plans={plans}
          onComplete={onComplete}
          onSkip={onSkip}
        />
      </div>
      </div>
    </div>
  );
}
