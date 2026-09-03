import { motion } from "motion/react";
import { useId } from "react";
import { cn } from "@/lib/cn";

export interface Segment<T extends string> {
  value: T;
  label: string;
  hint?: string;
  disabled?: boolean;
}

/**
 * The app's only multiple-choice control. Used for dictation behaviour,
 * processing style, and visual intensity so those three choices feel like the
 * same kind of decision.
 */
export function Segmented<T extends string>({
  value,
  segments,
  onChange,
  className,
}: {
  value: T;
  segments: Segment<T>[];
  onChange: (value: T) => void;
  className?: string;
}) {
  const layoutId = useId();

  return (
    <div
      role="radiogroup"
      className={cn(
        "no-drag inline-flex rounded-full border border-line bg-sunken p-[3px]",
        className,
      )}
    >
      {segments.map((segment) => {
        const selected = segment.value === value;
        return (
          <button
            key={segment.value}
            type="button"
            role="radio"
            aria-checked={selected}
            disabled={segment.disabled}
            title={segment.hint}
            onClick={() => onChange(segment.value)}
            className={cn(
              "relative flex-1 rounded-full px-3.5 py-1 text-[12.5px] transition-colors",
              "disabled:cursor-not-allowed disabled:text-line-2",
              selected ? "text-ink" : "text-ink-3 hover:text-ink-2",
            )}
          >
            {selected && (
              <motion.span
                layoutId={layoutId}
                transition={{ type: "spring", stiffness: 460, damping: 38 }}
                className="absolute inset-0 rounded-full bg-card shadow-[0_1px_2px_rgba(10,35,56,0.07)]"
              />
            )}
            <span className="relative z-10">{segment.label}</span>
          </button>
        );
      })}
    </div>
  );
}
