import type { ReactNode } from "react";
import { motion } from "motion/react";
import { cn } from "@/lib/cn";

/**
 * The bento tile. Every dashboard surface is one of these, which is what keeps
 * the grid reading as one object instead of a collection of panels.
 *
 * Border and background do the separating — no shadow, no gradient. Only the
 * hero card is allowed to lift off the page.
 */
interface CardProps {
  children: ReactNode;
  className?: string;
  /** The primary tile. Gets the one shadow in the app. */
  hero?: boolean;
  /** Stagger index for the entrance animation. */
  index?: number;
}

export function Card({ children, className, hero, index = 0 }: CardProps) {
  return (
    <motion.section
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{
        duration: 0.42,
        delay: Math.min(index, 6) * 0.04,
        ease: [0.22, 1, 0.36, 1],
      }}
      className={cn(
        "card",
        hero && "shadow-[0_1px_2px_rgba(10,35,56,0.04),0_10px_28px_-20px_rgba(10,35,56,0.22)]",
        className,
      )}
    >
      {children}
    </motion.section>
  );
}

export function CardHeader({
  label,
  action,
}: {
  label: string;
  action?: ReactNode;
}) {
  return (
    <header className="flex items-center justify-between gap-3">
      <h2 className="label">{label}</h2>
      {action}
    </header>
  );
}
