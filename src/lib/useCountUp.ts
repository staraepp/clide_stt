import { useEffect, useRef, useState } from "react";

/**
 * Animate a number toward its value.
 *
 * Only used on figures that are counts of real things — never on a number
 * invented to look impressive.
 */
export function useCountUp(target: number, durationMs = 620): number {
  const [value, setValue] = useState(0);
  const previous = useRef(0);

  useEffect(() => {
    if (
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      setValue(target);
      previous.current = target;
      return;
    }

    const from = previous.current;
    const start = performance.now();
    let frame = 0;

    const tick = (now: number) => {
      const progress = Math.min(1, (now - start) / durationMs);
      // Ease out: fast at first, settling gently — a counter that decelerates
      // reads as arriving rather than merely stopping.
      const eased = 1 - Math.pow(1 - progress, 3);
      setValue(Math.round(from + (target - from) * eased));

      if (progress < 1) {
        frame = requestAnimationFrame(tick);
      } else {
        previous.current = target;
      }
    };

    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [target, durationMs]);

  return value;
}
