import { useEffect, useRef, useState } from "react";
import { motion, useAnimationControls } from "motion/react";

import { cn } from "@/lib/cn";

const BASE = [0.38, 0.7, 1, 0.55, 0.32];

/**
 * The five-bar mark, alive.
 *
 * It is the same object in three states rather than three drawings: it breathes
 * while idle, becomes a **real level meter** while you are dictating, and
 * dances if you poke it. Making the brand mark the thing that shows your voice
 * is the one place clide's identity and its function are the same object.
 */
export function Wordmark({
  levelRef,
  live,
  className,
}: {
  /** Microphone level, 0..1. Read per frame; never causes a React render. */
  levelRef?: React.RefObject<number>;
  /** True while clide is actually hearing you. */
  live?: boolean;
  className?: string;
}) {
  const bars = useRef<(HTMLSpanElement | null)[]>([]);
  const controls = useAnimationControls();
  const [dancing, setDancing] = useState(false);
  const taps = useRef(0);
  const tapTimer = useRef<number | null>(null);

  // While live, drive heights straight from the meter — going through React
  // state here would re-render the title bar sixty times a second.
  useEffect(() => {
    if (!live || !levelRef) {
      bars.current.forEach((bar, index) => {
        if (bar) bar.style.height = `${BASE[index] * 100}%`;
      });
      return;
    }

    let frame = 0;
    const tick = () => {
      const level = levelRef.current ?? 0;
      bars.current.forEach((bar, index) => {
        if (!bar) return;
        // Each bar leans on a different part of the envelope so the mark reads
        // as a meter rather than five bars moving as one.
        const weight = BASE[index];
        const height = 18 + Math.min(1, level * (0.7 + weight * 0.9)) * 82;
        bar.style.height = `${height}%`;
      });
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [live, levelRef]);

  /** Poke it enough times and it dances. */
  const poke = () => {
    if (dancing) return;
    taps.current += 1;

    if (tapTimer.current) window.clearTimeout(tapTimer.current);
    tapTimer.current = window.setTimeout(() => {
      taps.current = 0;
    }, 1200);

    if (taps.current >= 5) {
      taps.current = 0;
      setDancing(true);
      controls
        .start({
          rotate: [0, -8, 8, -5, 0],
          scale: [1, 1.25, 0.92, 1.1, 1],
          transition: { duration: 0.9, ease: "easeInOut" },
        })
        .then(() => setDancing(false));
    }
  };

  return (
    <motion.span
      animate={controls}
      onClick={poke}
      aria-hidden
      className={cn(
        "flex h-[13px] cursor-pointer items-end gap-[1.5px]",
        className,
      )}
    >
      {BASE.map((height, index) => (
        <motion.span
          key={index}
          ref={(node) => {
            bars.current[index] = node;
          }}
          className="w-[2px] rounded-[1px] bg-voice"
          style={{ height: `${height * 100}%` }}
          animate={
            dancing
              ? { scaleY: [1, 1.8, 0.5, 1.4, 1] }
              : live
                ? {}
                : // Idle: a slow breath, so the mark is never quite still.
                  { scaleY: [1, 1.14, 1] }
          }
          transition={
            dancing
              ? { duration: 0.9, delay: index * 0.06, ease: "easeInOut" }
              : {
                  duration: 2.6,
                  repeat: Infinity,
                  ease: "easeInOut",
                  delay: index * 0.16,
                }
          }
        />
      ))}
    </motion.span>
  );
}
