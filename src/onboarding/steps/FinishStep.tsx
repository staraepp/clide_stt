import { Check } from "lucide-react";
import { motion } from "motion/react";
import { Keys } from "@/components/Keys";
import type { SystemStatus } from "@/lib/types";

export function FinishStep({ status }: { status: SystemStatus }) {
  return (
    <div className="text-center">
      <motion.div
        initial={{ scale: 0.5, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 320, damping: 20 }}
        className="mx-auto flex size-14 items-center justify-center rounded-full bg-ok/12 text-ok"
      >
        <Check size={26} strokeWidth={2.5} />
      </motion.div>

      <h1 className="display mt-6 text-[24px] text-ink">
        clide is ready
      </h1>
      <p className="mx-auto mt-3 max-w-[400px] text-[13.5px] leading-relaxed text-ink-2">
        Dictate from any app with <Keys accelerator={status.settings.shortcut} />.
        Everything you dictate is saved as text in your local history — the
        audio never is.
      </p>
    </div>
  );
}
