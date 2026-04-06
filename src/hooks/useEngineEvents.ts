import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

export interface SummaryEvent {
  kind: string;
  periodKey: string;
  text: string;
  trigger: string;
}

export interface FeedbackEvent {
  taskTitle: string;
  text: string;
}

export function useEngineEvents(
  onSummary: (p: SummaryEvent) => void,
  onFeedback: (p: FeedbackEvent) => void,
) {
  const sRef = useRef(onSummary);
  const fRef = useRef(onFeedback);
  sRef.current = onSummary;
  fRef.current = onFeedback;

  useEffect(() => {
    const un: (() => void)[] = [];
    let cancelled = false;
    (async () => {
      const a = await listen<SummaryEvent>("engine/summary", (e) => {
        if (!cancelled && e.payload) sRef.current(e.payload);
      });
      const b = await listen<FeedbackEvent>("engine/feedback", (e) => {
        if (!cancelled && e.payload) fRef.current(e.payload);
      });
      un.push(() => a(), () => b());
    })();
    return () => {
      cancelled = true;
      un.forEach((f) => f());
    };
  }, []);
}
