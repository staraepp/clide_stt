/**
 * clide's motion vocabulary.
 *
 * The product rule is in `blueprint.md`: *while working, clide disappears; when
 * opened, clide comes alive.* The HUD and the dictation path stay severe. This
 * file is the other half — the main window is allowed to feel responsive.
 *
 * Everything here is short. Motion that makes you wait is not delight, it is
 * latency wearing a costume. Nothing exceeds ~320ms, and anything on the
 * critical path of a click resolves in half that.
 *
 * `prefers-reduced-motion` is honoured globally in `theme.css`, which collapses
 * every duration — so these values do not need to check it individually.
 */

import type { Transition, Variants } from "motion/react";

/** The app's one spring. Used for anything that should feel physical. */
export const SPRING: Transition = {
  type: "spring",
  stiffness: 460,
  damping: 34,
  mass: 0.7,
};

/** Snappier, for things that must not lag a click. */
export const SPRING_QUICK: Transition = {
  type: "spring",
  stiffness: 620,
  damping: 30,
  mass: 0.5,
};

/** For opacity and position, where a spring would look wobbly. */
export const EASE: Transition = {
  duration: 0.26,
  ease: [0.22, 1, 0.36, 1],
};

export const EASE_FAST: Transition = {
  duration: 0.16,
  ease: [0.22, 1, 0.36, 1],
};

/**
 * Press feedback. The single biggest reason an interface feels alive: the
 * thing you touch acknowledges you before anything else happens.
 */
export const PRESS = {
  whileHover: { scale: 1.015 },
  whileTap: { scale: 0.975 },
  transition: SPRING_QUICK,
} as const;

/** For larger surfaces, where the same scale would look rubbery. */
export const PRESS_SUBTLE = {
  whileHover: { scale: 1.005 },
  whileTap: { scale: 0.995 },
  transition: SPRING_QUICK,
} as const;

/** Cards rise very slightly toward the pointer. */
export const LIFT = {
  whileHover: { y: -2 },
  transition: SPRING_QUICK,
} as const;

/** Entrance for a list or grid. Stagger by index, capped so long lists don't crawl. */
export function enter(index = 0): Variants {
  return {
    hidden: { opacity: 0, y: 10 },
    shown: {
      opacity: 1,
      y: 0,
      transition: { ...EASE, delay: Math.min(index, 8) * 0.035 },
    },
  };
}

/**
 * The one celebratory beat in the app, for the moment a transcript lands.
 *
 * Deliberately singular: if everything celebrates, nothing does.
 */
export const LAND: Variants = {
  hidden: { opacity: 0, y: 6, scale: 0.985 },
  shown: {
    opacity: 1,
    y: 0,
    scale: 1,
    transition: { type: "spring", stiffness: 520, damping: 26, mass: 0.6 },
  },
};
