import { clsx } from "clsx";
import ReactMarkdown from "react-markdown";

export interface ToastItem {
  id: string;
  title: string;
  body: string;
  variant: "summary" | "feedback";
}

export function ToastStack({
  items,
  onDismiss,
}: {
  items: ToastItem[];
  onDismiss: (id: string) => void;
}) {
  return (
    <div className="fixed bottom-4 right-4 z-50 flex max-h-[70vh] w-[min(420px,92vw)] flex-col gap-2 overflow-y-auto">
      {items.map((t) => (
        <div
          key={t.id}
          className={clsx(
            "cv-auto aura-panel rounded-xl p-4 shadow-lg backdrop-blur-sm",
            t.variant === "summary"
              ? "border-[rgba(109,155,118,0.25)]"
              : "border-[rgba(120,160,200,0.22)]",
          )}
        >
          <div className="mb-2 flex items-start justify-between gap-2">
            <span className="text-[10px] font-semibold uppercase tracking-wide text-[var(--aura-muted)]">
              {t.title}
            </span>
            <button
              type="button"
              className="rounded px-1 text-[var(--aura-muted)] hover:bg-white/5 hover:text-[var(--aura-text)]"
              onClick={() => onDismiss(t.id)}
            >
              ✕
            </button>
          </div>
          <div className="prose prose-invert prose-sm prose-p:my-1 max-w-none text-[var(--aura-text)] prose-headings:my-2 prose-headings:text-[var(--aura-text)]">
            <ReactMarkdown>{t.body}</ReactMarkdown>
          </div>
        </div>
      ))}
    </div>
  );
}
