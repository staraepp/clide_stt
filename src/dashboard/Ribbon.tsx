import { AnimatePresence, motion } from "motion/react";
import type { RefObject } from "react";

import { Waveform } from "@/components/Waveform";
import { StatusDot } from "@/components/StatusDot";
import { stateTone, stateLabel, failureDetail } from "@/dictation/labels";
import { transcriptOf, type DictationState } from "@/lib/types";
import { cn } from "@/lib/cn";

/**
 * The ribbon — the one element that is the whole product.
 *
 * It does not swap between four different panels; it is a single surface that
 * changes what it holds: a prompt at rest, the live meter while listening, the
 * transcript once it arrives, the reason if something failed. Keeping it as one
 * object is what makes the dashboard feel like the dictation rather than a
 * readout about it.
 */
export function Ribbon({
  state,
  levelRef,
  shortcut,
}: {
  state: DictationState;
  levelRef: RefObject<number>;
  shortcut: string;
}) {
  const failure = failureDetail(state);
  const transcript = transcriptOf(state);
  const listening = state.kind === "capturing";
  const settledEmpty = state.kind === "idle";

  const surface = failure
    ? "bg-stop-tint"
    : state.kind === "complete"
      ? "bg-ok-tint"
      : settledEmpty
        ? "bg-sunken border-line"
        : "bg-voice-tint";

  return (
    <div
      className={cn(
        "mt-4 flex min-h-[152px] items-center rounded-xl border border-transparent px-5 py-5",
        "transition-colors duration-500",
        surface,
      )}
    >
      <div className="w-full">
        <div className="mb-3 flex items-center gap-2">
          <StatusDot tone={stateTone(state)} pulse={!settledEmpty && !failure && !transcript} />
          <span
            className={cn(
              "label",
              failure && "text-stop",
              state.kind === "complete" && "text-ok",
              listening && "text-voice-deep",
            )}
          >
            {settledEmpty ? "Ready" : stateLabel(state)}
          </span>
        </div>

        <AnimatePresence mode="wait" initial={false}>
          {listening ? (
            <motion.div
              key="wave"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.18 }}
              className="h-9"
            >
              <Waveform levelRef={levelRef} bars={46} />
            </motion.div>
          ) : (
            <motion.p
              key={failure ? "failure" : transcript ? "transcript" : "prompt"}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
              className={cn(
                "flex min-h-9 items-center text-[16px] leading-relaxed",
                failure ? "text-ink-2" : transcript ? "text-ink" : "text-ink-3",
              )}
            >
              {failure ??
                transcript ??
                `Hold ${shortcut} anywhere and speak.`}
            </motion.p>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
