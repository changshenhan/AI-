import { memo, useEffect, useMemo, useState } from "react";
import { clsx } from "clsx";

export interface BusyDto {
  id: string;
  startAt: string;
  endAt: string;
  label?: string | null;
  sourceMessageId?: string | null;
}

export interface PlanDto {
  id: string;
  day: string;
  title: string;
  startAt: string;
  endAt: string;
  status: string;
  sourceConversationId?: string | null;
}

function traceHint(raw?: string | null): string | undefined {
  if (!raw) return undefined;
  const id = raw.startsWith("trace:") ? raw.slice(6) : raw;
  if (!id) return undefined;
  return id.length <= 8 ? `溯源 ${id}` : `溯源 …${id.slice(-8)}`;
}

/** 本地日历日 0:00–24:00 的分钟数（与浏览器时区一致） */
function minsFromLocalDay(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return 0;
  return d.getHours() * 60 + d.getMinutes() + d.getSeconds() / 60;
}

const DAY = 24 * 60;

/** 日视图总高度：每小时约 ≥54px，避免挤在一起 */
const TIMELINE_HEIGHT = "min(1320px, 92vh)";

const HOURS = Array.from({ length: 25 }, (_, i) => i);

const Block = memo(function Block({
  topPct,
  heightPct,
  className,
  label,
  sub,
}: {
  topPct: number;
  heightPct: number;
  className: string;
  label: string;
  sub?: string;
}) {
  return (
    <div
      className={clsx(
        "absolute left-1 right-1 overflow-hidden rounded-md border px-2 py-1.5 text-[13px] leading-snug",
        className,
      )}
      style={{
        top: `${topPct}%`,
        height: `${Math.max(heightPct, 0.8)}%`,
        contentVisibility: "auto",
      }}
    >
      <div className="font-medium">{label}</div>
      {sub && <div className="truncate opacity-80">{sub}</div>}
    </div>
  );
});

function useNowPercentForDay(viewDay: string, tickMs = 30000) {
  const [pct, setPct] = useState<number | null>(null);

  useEffect(() => {
    const run = () => {
      const now = new Date();
      const y = now.getFullYear();
      const mo = String(now.getMonth() + 1).padStart(2, "0");
      const da = String(now.getDate()).padStart(2, "0");
      const todayStr = `${y}-${mo}-${da}`;
      if (viewDay.slice(0, 10) !== todayStr) {
        setPct(null);
        return;
      }
      const mins = now.getHours() * 60 + now.getMinutes() + now.getSeconds() / 60;
      setPct((mins / DAY) * 100);
    };
    run();
    const id = window.setInterval(run, tickMs);
    return () => window.clearInterval(id);
  }, [viewDay, tickMs]);

  return pct;
}

export const DayTimeline = memo(function DayTimeline({
  busy,
  plans,
  onComplete,
  onSkip,
  viewDay,
}: {
  busy: BusyDto[];
  plans: PlanDto[];
  onComplete: (id: string) => void;
  onSkip: (id: string) => void;
  /** 当前查看的日历日 YYYY-MM-DD，用于「此刻」线与刻度 */
  viewDay: string;
}) {
  const nowPct = useNowPercentForDay(viewDay);

  const layout = useMemo(() => {
    const b = busy.map((x) => {
      const s = minsFromLocalDay(x.startAt);
      const e = minsFromLocalDay(x.endAt);
      return {
        id: x.id,
        top: (s / DAY) * 100,
        h: ((e - s) / DAY) * 100,
        label: x.label || "不可用",
        trace: traceHint(x.sourceMessageId),
      };
    });
    const p = plans
      .filter((x) => x.status !== "skipped")
      .map((x) => {
        const s = minsFromLocalDay(x.startAt);
        const e = minsFromLocalDay(x.endAt);
        return {
          id: x.id,
          top: (s / DAY) * 100,
          h: ((e - s) / DAY) * 100,
          title: x.title,
          status: x.status,
          trace: traceHint(x.sourceConversationId),
        };
      });
    return { b, p };
  }, [busy, plans]);

  return (
    <div
      className="flex w-full select-none rounded-2xl border border-[var(--aura-border)] bg-[var(--aura-bg-elevated)]"
      style={{ height: TIMELINE_HEIGHT }}
    >
      {/* 左侧：固定宽度时间刻度，避免与负 margin 冲突导致只显示 00 */}
      <div className="relative w-[3rem] shrink-0 border-r border-[var(--aura-border)]">
        {HOURS.map((h) => (
          <div
            key={`label-${h}`}
            className="pointer-events-none absolute right-1.5 text-[10px] tabular-nums leading-none text-[var(--aura-muted)]"
            style={{
              top: `calc(${(h / 24) * 100}% - 0.35em)`,
            }}
          >
            {`${String(h).padStart(2, "0")}:00`}
          </div>
        ))}
      </div>

      {/* 右侧：网格线 + 块 + 此刻 */}
      <div className="relative min-h-0 min-w-0 flex-1">
        {HOURS.map((h) => (
          <div
            key={`grid-${h}`}
            className="pointer-events-none absolute left-0 right-0 border-t border-[var(--aura-border)]"
            style={{ top: `${(h / 24) * 100}%` }}
          />
        ))}

        {nowPct != null && (
          <div
            className="pointer-events-none absolute left-0 right-0 z-10 border-t-2 border-[var(--aura-accent)]"
            style={{ top: `${nowPct}%` }}
            title="此刻"
          />
        )}

        <div className="absolute inset-0">
          {layout.b.map((x) => (
            <Block
              key={x.id}
              topPct={x.top}
              heightPct={x.h}
              className="border-rose-500/50 bg-rose-950/80 text-rose-100"
              label={x.label}
              sub={[x.trace, "不可用"].filter(Boolean).join(" · ")}
            />
          ))}
          {layout.p.map((x) => (
            <div key={x.id} className="pointer-events-none absolute inset-0">
              <div className="pointer-events-auto">
                <Block
                  topPct={x.top}
                  heightPct={x.h}
                  className={clsx(
                    "border-emerald-500/40",
                    x.status === "done"
                      ? "bg-zinc-800/90 line-through opacity-60"
                      : "bg-emerald-950/85 text-emerald-50",
                  )}
                  label={x.title}
                  sub={[x.trace, x.status].filter(Boolean).join(" · ")}
                />
              </div>
              {x.status === "pending" && (
                <div
                  className="pointer-events-auto absolute right-1 z-20 flex flex-col gap-0.5"
                  style={{ top: `calc(${x.top}% + 2px)` }}
                >
                  <button
                    type="button"
                    className="rounded bg-emerald-600 px-2 py-0.5 text-[10px] font-medium text-white hover:bg-emerald-500"
                    onClick={() => onComplete(x.id)}
                  >
                    完成
                  </button>
                  <button
                    type="button"
                    className="rounded border border-zinc-600 bg-zinc-900/90 px-2 py-0.5 text-[10px] text-zinc-300 hover:bg-zinc-800"
                    onClick={() => onSkip(x.id)}
                  >
                    跳过
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
});
