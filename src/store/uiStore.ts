import { create } from "zustand";

export type TabId = "planner" | "calendar" | "summaries" | "data";

function todayISO() {
  return new Date().toISOString().slice(0, 10);
}

export const useUiStore = create<{
  tab: TabId;
  setTab: (t: TabId) => void;
  selectedDay: string;
  setSelectedDay: (d: string) => void;
  /** 排期等操作后自增，供日历页重新拉取月视图 */
  calendarRevision: number;
  bumpCalendar: () => void;
}>()((set) => ({
  tab: "planner",
  setTab: (tab) => set({ tab }),
  selectedDay: todayISO(),
  setSelectedDay: (selectedDay) => set({ selectedDay }),
  calendarRevision: 0,
  bumpCalendar: () =>
    set((s) => ({ calendarRevision: s.calendarRevision + 1 })),
}));
