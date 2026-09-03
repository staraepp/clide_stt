import { useEffect, useRef, useState } from "react";
import { Sparkles } from "lucide-react";

import { Keys } from "@/components/Keys";
import { StatusDot } from "@/components/StatusDot";
import { useDictationState } from "@/dictation/useDictationState";
import { stateLabel, stateTone, failureDetail } from "@/dictation/labels";
import type { SystemStatus } from "@/lib/types";
import { cn } from "@/lib/cn";
import { StepLayout } from "../StepLayout";

/**
 * The real thing.
 *
 * This field is a plain textarea and the dictation runs the actual pipeline —
 * shortcut, microphone, provider, processing, insertion. Nothing here is
 * simulated: if the text appears, the whole path works, and if it doesn't, the
 * failure the user sees is the one they would have hit later anyway.
 */
export function TestDictationStep({
  status,
  onSuccess,
}: {
  status: SystemStatus;
  onSuccess: () => void;
}) {
  const state = useDictationState();
  const [value, setValue] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const succeeded = value.trim().length > 0;

  // Focus the field so the shortcut has somewhere to insert into.
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  useEffect(() => {
    if (succeeded) onSuccess();
  }, [succeeded, onSuccess]);

  const failure = failureDetail(state);

  return (
    <StepLayout
      icon={<Sparkles size={18} />}
      title="Try it once"
      description="Click into the box, then use your shortcut and say a sentence. This runs the real pipeline, not a demo."
    >
      <div className="flex items-center gap-2.5 text-[12.5px] text-ink-2">
        <span>
          {status.settings.behavior === "hold" ? "Hold" : "Press"}
        </span>
        <Keys accelerator={status.settings.shortcut} />
        <span>and speak</span>
      </div>

      <textarea
        ref={textareaRef}
        value={value}
        onChange={(event) => setValue(event.target.value)}
        rows={3}
        placeholder="Your words will land here…"
        className={cn(
          "no-drag mt-3 w-full resize-none rounded-xl border bg-sunken p-3.5",
          "text-[13.5px] leading-relaxed text-ink placeholder:text-ink-2/60",
          "outline-none transition-colors",
          succeeded
            ? "border-ok/50"
            : "border-line focus:border-voice",
        )}
      />

      <div className="mt-3 flex items-center gap-2">
        <StatusDot tone={succeeded ? "ready" : stateTone(state)} />
        <span
          className={cn(
            "text-[12.5px]",
            failure ? "text-stop" : "text-ink-2",
          )}
        >
          {failure ??
            (succeeded
              ? "That's the whole pipeline working."
              : state.kind === "idle"
                ? "Waiting for your shortcut."
                : stateLabel(state))}
        </span>
      </div>
    </StepLayout>
  );
}
