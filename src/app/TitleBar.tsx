import { motion } from "motion/react";
import { StatusDot } from "@/components/StatusDot";
import { Wordmark } from "@/components/Wordmark";
import { stateLabel, stateTone } from "@/dictation/labels";
import { isBusy, type DictationState, type SystemStatus } from "@/lib/types";
import { cn } from "@/lib/cn";

export type View = "dashboard" | "models" | "history" | "settings";

const TABS: { value: View; label: string }[] = [
  { value: "dashboard", label: "Dashboard" },
  { value: "models", label: "Models" },
  { value: "history", label: "History" },
  { value: "settings", label: "Settings" },
];

/**
 * Custom title bar.
 *
 * The window uses macOS's overlay title bar style, so the traffic lights float
 * over this row — hence the left inset. Dragging works anywhere in the bar
 * except on the controls.
 */
export function TitleBar({
  view,
  onChange,
  status,
  state,
  levelRef,
}: {
  view: View;
  onChange: (view: View) => void;
  status: SystemStatus;
  state: DictationState;
  levelRef: React.RefObject<number>;
}) {
  const busy = isBusy(state);
  const label =
    state.kind === "idle"
      ? status.ready
        ? "Ready"
        : "Setup needed"
      : stateLabel(state);

  return (
    <header
      // Tauri's own handler, not the CSS property: `-webkit-app-region` is
      // silently ignored by this webview, which left only the native overlay
      // strip draggable and made the window feel stuck.
      data-tauri-drag-region
      className="drag-region flex h-[50px] shrink-0 items-center gap-4 border-b border-line bg-card px-5 pl-[88px]"
    >
      <span
        data-tauri-drag-region
        className="display flex items-center gap-2 text-[14.5px]"
      >
        <Wordmark levelRef={levelRef} live={state.kind === "capturing"} />
        clide
      </span>

      <nav className="no-drag flex items-center gap-0.5">
        {TABS.map((tab) => {
          const active = tab.value === view;
          return (
            <button
              key={tab.value}
              type="button"
              onClick={() => onChange(tab.value)}
              aria-current={active ? "page" : undefined}
              className={cn(
                "relative rounded-lg px-2.5 py-1 text-[13px] transition-colors",
                active ? "text-ink" : "text-ink-3 hover:text-ink-2",
              )}
            >
              {active && (
                <motion.span
                  layoutId="tab"
                  transition={{ type: "spring", stiffness: 460, damping: 38 }}
                  className="absolute inset-0 rounded-lg bg-voice-tint"
                />
              )}
              <span className="relative z-10">{tab.label}</span>
            </button>
          );
        })}
      </nav>

      <div data-tauri-drag-region className="ml-auto flex items-center gap-2">
        <StatusDot
          tone={state.kind === "idle" && !status.ready ? "pending" : stateTone(state)}
          pulse={busy}
        />
        <span className="text-[12.5px] text-ink-2">{label}</span>
      </div>
    </header>
  );
}
