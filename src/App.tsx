import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SetupView, type LlmSettingsPayload } from "./components/SetupView";
import { ToastStack, type ToastItem } from "./components/ToastStack";
import { useEngineEvents } from "./hooks/useEngineEvents";
import { useUiStore } from "./store/uiStore";

const PlannerPage = lazy(() =>
  import("./pages/PlannerPage").then((m) => ({ default: m.PlannerPage })),
);
const CalendarPage = lazy(() =>
  import("./pages/CalendarPage").then((m) => ({ default: m.CalendarPage })),
);
const SummariesPage = lazy(() =>
  import("./pages/SummariesPage").then((m) => ({ default: m.SummariesPage })),
);
const DataPage = lazy(() =>
  import("./pages/DataPage").then((m) => ({ default: m.DataPage })),
);

function ShellSpinner() {
  return (
    <div className="flex flex-1 items-center justify-center text-[var(--aura-muted)]">
      加载模块…
    </div>
  );
}

export default function App() {
  const [ready, setReady] = useState(false);
  const [settings, setSettings] = useState<LlmSettingsPayload | null>(null);
  const { tab, setTab } = useUiStore();
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const [tz, setTz] = useState<string | null>(null);

  const pushToast = useCallback((t: Omit<ToastItem, "id">) => {
    const id = crypto.randomUUID();
    setToasts((x) => [...x, { ...t, id }]);
  }, []);

  const onSummary = useCallback(
    (p: { kind: string; periodKey: string; text: string; trigger: string }) => {
      pushToast({
        title: `${p.kind === "daily" ? "日总结" : "周总结"} · ${p.periodKey}`,
        body: p.text,
        variant: "summary",
      });
    },
    [pushToast],
  );

  const onFeedback = useCallback((_p: { taskTitle: string; text: string }) => {
    // 完成鼓励由 `complete_plan_item` 的返回值在日历页内联展示（含时效与任务名）；
    // 若再弹右下角会与日历卡片重复，故此处不叠 Toast。
  }, []);

  useEngineEvents(onSummary, onFeedback);

  useEffect(() => {
    void invoke<string>("system_timezone").then(setTz).catch(() => setTz(null));
  }, []);

  useEffect(() => {
    void invoke<LlmSettingsPayload | null>("llm_load_settings").then((s) => {
      if (s?.providerId && s.model) {
        setSettings({
          providerId: s.providerId,
          apiKey: s.apiKey,
          baseUrlOverride: s.baseUrlOverride ?? "",
          model: s.model,
        });
      }
      setReady(true);
    });
  }, []);

  const tabs = useMemo(
    () =>
      [
        { id: "planner" as const, label: "规划与对话" },
        { id: "calendar" as const, label: "日历" },
        { id: "summaries" as const, label: "总结" },
        { id: "data" as const, label: "数据" },
      ] as const,
    [],
  );

  if (!ready) {
    return (
      <div className="flex min-h-screen items-center justify-center text-[var(--aura-muted)]">
        初始化引擎…
      </div>
    );
  }

  if (!settings) {
    return (
      <div className="mx-auto flex min-h-screen max-w-lg flex-col justify-center px-4 py-10">
        <SetupView initial={null} onSaved={setSettings} />
      </div>
    );
  }

  return (
    <div className="mx-auto flex min-h-screen max-w-6xl flex-col px-4 py-5">
      <header className="mb-5 flex flex-wrap items-center justify-between gap-4 border-b border-[var(--aura-border)] pb-4">
        <div>
          <h1 className="text-lg font-semibold tracking-tight text-[var(--aura-text)]">
            AI 日程引擎
          </h1>
          <p className="mt-1 max-w-md text-xs leading-relaxed text-[var(--aura-muted)]">
            本地 SQLite · 主进程调度 · 工具化总结
            {tz ? (
              <span className="mt-0.5 block text-[var(--aura-muted)] opacity-80">
                系统时区 · {tz}
              </span>
            ) : null}
          </p>
        </div>
        <nav className="flex flex-wrap items-center gap-1.5">
          {tabs.map((t) => (
            <button
              key={t.id}
              type="button"
              data-active={tab === t.id}
              onClick={() => setTab(t.id)}
              className="aura-nav-pill"
            >
              {t.label}
            </button>
          ))}
          <button
            type="button"
            className="aura-btn aura-btn-ghost ml-1 text-[var(--aura-muted)]"
            onClick={() => {
              void invoke("llm_clear_settings");
              setSettings(null);
            }}
          >
            退出 API
          </button>
        </nav>
      </header>

      <Suspense fallback={<ShellSpinner />}>
        {tab === "planner" && <PlannerPage settings={settings} />}
        {tab === "calendar" && <CalendarPage />}
        {tab === "summaries" && <SummariesPage />}
        {tab === "data" && <DataPage />}
      </Suspense>

      <ToastStack
        items={toasts}
        onDismiss={(id) => setToasts((x) => x.filter((t) => t.id !== id))}
      />
    </div>
  );
}
