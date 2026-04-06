import { useVirtualizer } from "@tanstack/react-virtual";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  memo,
  useCallback,
  useDeferredValue,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import type { LlmSettingsPayload } from "../components/SetupView";
import { stripChatAsterisks } from "../lib/chatDisplay";
import { useUiStore } from "../store/uiStore";

type Msg = {
  role: "user" | "assistant";
  content: string;
  /** 仅左侧对话流为 true；自然语言排期生成的 assistant 为 false，不滤星号 */
  streamChat?: boolean;
};

const ChatRow = memo(function ChatRow({
  msg,
  style,
}: {
  msg: Msg;
  style: CSSProperties;
}) {
  const isUser = msg.role === "user";
  const raw = msg.content;
  const display =
    !isUser && msg.streamChat !== false
      ? stripChatAsterisks(raw)
      : raw;

  return (
    <div
      className="absolute left-0 top-0 w-full px-2 py-1.5"
      style={style}
    >
      <div
        className={
          isUser
            ? "ml-7 rounded-xl border border-[var(--aura-border)] bg-white/[0.04] px-3 py-2 text-[0.8125rem] leading-relaxed text-[var(--aura-text)]"
            : "aura-panel mr-7 max-w-none rounded-xl px-3 py-2"
        }
      >
        <span className="mb-0.5 block text-[10px] font-medium uppercase tracking-wider text-[var(--aura-muted)]">
          {isUser ? "你" : "模型"}
        </span>
        <div className="chat-bubble-assistant whitespace-pre-wrap text-[var(--aura-text)]">
          {display}
        </div>
      </div>
    </div>
  );
});

export function PlannerPage({ settings }: { settings: LlmSettingsPayload }) {
  const bumpCalendar = useUiStore((s) => s.bumpCalendar);
  const [messages, setMessages] = useState<Msg[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [nl, setNl] = useState("");
  const [day, setDay] = useState(() => new Date().toISOString().slice(0, 10));
  const [nlBusy, setNlBusy] = useState(false);
  const [engineNow, setEngineNow] = useState<string | null>(null);
  const parentRef = useRef<HTMLDivElement>(null);
  const defNl = useDeferredValue(nl);

  useEffect(() => {
    const tick = () => {
      void invoke<string>("time_now_iso")
        .then((iso) => {
          const d = new Date(iso);
          if (Number.isNaN(d.getTime())) {
            setEngineNow(iso.slice(0, 19));
            return;
          }
          setEngineNow(
            d.toLocaleString(undefined, {
              year: "numeric",
              month: "2-digit",
              day: "2-digit",
              hour: "2-digit",
              minute: "2-digit",
              second: "2-digit",
              hour12: false,
            }),
          );
        })
        .catch(() => setEngineNow(null));
    };
    tick();
    const id = window.setInterval(tick, 20000);
    return () => window.clearInterval(id);
  }, []);

  const rowVirtual = useVirtualizer({
    count: messages.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 76,
    overscan: 5,
  });

  const send = useCallback(async () => {
    const t = input.trim();
    if (!t || loading) return;
    setInput("");
    setLoading(true);
    const next: Msg[] = [
      ...messages,
      { role: "user", content: t, streamChat: true },
      { role: "assistant", content: "", streamChat: true },
    ];
    setMessages(next);
    const payload = {
      settings: {
        providerId: settings.providerId.trim(),
        apiKey: settings.apiKey.trim(),
        baseUrlOverride: settings.baseUrlOverride.trim() || null,
        model: settings.model.trim(),
      },
      messages: next.slice(0, -1).map((m) => ({
        role: m.role,
        content: m.content,
      })),
    };
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<{ delta: string; done: boolean }>(
        "llm/stream",
        (event) => {
          const p = event.payload;
          const chunk = stripChatAsterisks(p.delta ?? "");
          setMessages((prev) => {
            const out = [...prev];
            const i = out.length - 1;
            if (i >= 0 && out[i].role === "assistant") {
              out[i] = {
                ...out[i],
                content: out[i].content + chunk,
              };
            }
            return out;
          });
        },
      );
      await invoke("llm_chat_stream", payload);
    } catch (e) {
      setMessages((prev) => {
        const out = [...prev];
        const i = out.length - 1;
        if (i >= 0 && out[i].role === "assistant") {
          const errText = `错误：${String(e)}`;
          out[i] = {
            ...out[i],
            content:
              out[i].content.trim() === ""
                ? errText
                : `${out[i].content}\n\n（${errText}）`,
          };
        }
        return out;
      });
    } finally {
      unlisten?.();
      setLoading(false);
    }
  }, [input, loading, messages, settings]);

  const applyNl = useCallback(async () => {
    const t = defNl.trim();
    if (!t || nlBusy) return;
    setNlBusy(true);
    try {
      const traceId = crypto.randomUUID();
      const r = await invoke<{
        note: string;
        busyInserted: number;
        plansInserted: number;
        traceId: string;
      }>("nlp_apply_plan", {
        settings: {
          providerId: settings.providerId.trim(),
          apiKey: settings.apiKey.trim(),
          baseUrlOverride: settings.baseUrlOverride.trim() || null,
          model: settings.model.trim(),
        },
        day,
        userText: t,
        traceId,
      });
      bumpCalendar();
      setMessages((m) => [
        ...m,
        {
          role: "assistant",
          content: `排期已应用：${r.note}（红块 ${r.busyInserted}，计划 ${r.plansInserted}；溯源 ${r.traceId.slice(0, 8)}…）`,
          streamChat: false,
        },
      ]);
      setNl("");
    } catch (e) {
      setMessages((m) => [
        ...m,
        {
          role: "assistant",
          content: `排期失败：${String(e)}`,
          streamChat: false,
        },
      ]);
    } finally {
      setNlBusy(false);
    }
  }, [day, defNl, nlBusy, bumpCalendar, settings]);

  useEffect(() => {
    if (messages.length === 0) return;
    requestAnimationFrame(() => {
      rowVirtual.scrollToIndex(messages.length - 1, { align: "end" });
    });
  }, [messages.length, rowVirtual]);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 lg:flex-row">
      <section className="aura-panel flex min-h-[320px] min-w-0 flex-1 flex-col rounded-2xl">
        <header className="border-b border-[var(--aura-border)] px-4 py-2.5 text-[11px] font-medium uppercase tracking-wide text-[var(--aura-muted)]">
          对话 · 流式
        </header>
        <div
          ref={parentRef}
          className="min-h-0 flex-1 overflow-y-auto scroll-smooth px-1 py-2"
        >
          <div
            className="relative"
            style={{ height: `${rowVirtual.getTotalSize()}px` }}
          >
            {rowVirtual.getVirtualItems().map((vi) => {
              const msg = messages[vi.index];
              return (
                <ChatRow
                  key={vi.key}
                  msg={msg}
                  style={{
                    transform: `translateY(${vi.start}px)`,
                  }}
                />
              );
            })}
          </div>
        </div>
        <form
          className="flex gap-2 border-t border-[var(--aura-border)] p-3"
          onSubmit={(e) => {
            e.preventDefault();
            void send();
          }}
        >
          <input
            className="aura-input min-w-0 flex-1"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="发送消息…"
            disabled={loading}
            autoComplete="off"
          />
          <button
            type="submit"
            disabled={loading}
            className="aura-btn aura-btn-primary min-w-[4.5rem] disabled:opacity-45"
          >
            {loading ? "…" : "发送"}
          </button>
        </form>
      </section>

      <aside className="w-full shrink-0 space-y-3 lg:w-[19rem]">
        <div className="aura-panel rounded-2xl p-4">
          <h3 className="mb-1 text-sm font-semibold tracking-tight text-[var(--aura-text)]">
            自然语言排期
          </h3>
          <p className="mb-3 font-mono text-[10px] leading-snug text-[var(--aura-muted)]">
            引擎本地现在：{engineNow ?? "…"}（每 20s 同步，排期锚定此时刻）
          </p>
          <label className="mb-1.5 block text-[11px] font-medium text-[var(--aura-muted)]">
            目标日
          </label>
          <input
            type="date"
            className="aura-input mb-3 w-full"
            value={day}
            onChange={(e) => setDay(e.target.value)}
          />
          <textarea
            className="aura-input mb-3 min-h-[120px] w-full resize-y"
            placeholder="描述不可用时间与待办，例如：上午 9–12 会议；下午写报告…"
            value={nl}
            onChange={(e) => setNl(e.target.value)}
          />
          <button
            type="button"
            disabled={nlBusy}
            onClick={() => void applyNl()}
            className="aura-btn w-full rounded-xl border border-[var(--aura-border-strong)] bg-[rgba(201,168,108,0.1)] py-2.5 text-sm font-medium text-[var(--aura-warm)] transition-colors hover:bg-[rgba(201,168,108,0.16)] disabled:opacity-45"
          >
            {nlBusy ? "写入中…" : "生成日历块"}
          </button>
        </div>
      </aside>
    </div>
  );
}
