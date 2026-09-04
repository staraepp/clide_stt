import { motion } from "motion/react";
import { SPRING_QUICK } from "@/lib/motion";
import { cn } from "@/lib/cn";

/**
 * An on/off switch.
 *
 * Used where a setting is genuinely binary and the consequence is worth being
 * explicit about — turning a cloud refiner on means transcripts leave the Mac,
 * and a switch says that more honestly than a list you happen to be selected in.
 */
export function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "no-drag relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full border transition-colors",
        "disabled:cursor-not-allowed disabled:opacity-40",
        checked ? "border-voice bg-voice" : "border-line-2 bg-sunken",
      )}
    >
      <motion.span
        layout
        transition={SPRING_QUICK}
        className={cn(
          "block size-[16px] rounded-full bg-card shadow-[0_1px_2px_rgba(10,35,56,0.2)]",
          checked ? "ml-[19px]" : "ml-[2px]",
        )}
      />
    </button>
  );
}
