import { FIT_LABEL, type Fit } from "@/lib/types";
import { cn } from "@/lib/cn";

/**
 * How well a model suits this machine.
 *
 * Deliberately worded as a verdict about *this Mac* rather than a generic
 * quality score — it is the answer to "should I download this one?"
 */
const TONE: Record<Fit, string> = {
  great: "bg-ok/10 text-ok",
  good: "bg-ok/10 text-ok",
  tight: "bg-warn/12 text-warn",
  tooLarge: "bg-stop/10 text-stop",
};

export function FitBadge({ fit, className }: { fit: Fit; className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium",
        TONE[fit],
        className,
      )}
    >
      {FIT_LABEL[fit]}
    </span>
  );
}
