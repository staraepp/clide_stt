import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Check, Copy } from "lucide-react";

import { Button } from "./Button";
import { SPRING_QUICK } from "@/lib/motion";
import * as commands from "@/lib/commands";
import { cn } from "@/lib/cn";

/**
 * Copy, with the confirmation the action deserves.
 *
 * A copy that gives no feedback leaves you wondering whether it worked, so the
 * icon swaps to a tick and holds long enough to be seen before reverting.
 */
export function CopyButton({
  text,
  label,
  size = "sm",
  variant = "ghost",
  className,
}: {
  text: string;
  label?: string;
  size?: "sm" | "md";
  variant?: "ghost" | "surface";
  className?: string;
}) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1400);
    return () => clearTimeout(timer);
  }, [copied]);

  return (
    <Button
      size={size}
      variant={variant}
      className={cn(copied && "text-ok", className)}
      aria-label={label ?? "Copy"}
      onClick={async () => {
        await commands.copyText(text);
        setCopied(true);
      }}
    >
      <span className="relative flex size-3.5 items-center justify-center">
        <AnimatePresence mode="wait" initial={false}>
          <motion.span
            key={copied ? "done" : "idle"}
            initial={{ scale: 0.5, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.5, opacity: 0 }}
            transition={SPRING_QUICK}
            className="absolute inset-0 flex items-center justify-center"
          >
            {copied ? <Check size={13} strokeWidth={3} /> : <Copy size={12} />}
          </motion.span>
        </AnimatePresence>
      </span>
      {label && <span>{copied ? "Copied" : label}</span>}
    </Button>
  );
}
