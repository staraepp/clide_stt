import { motion } from "motion/react";

export function WelcomeStep() {
  return (
    <div className="text-center">
      <motion.div
        initial={{ scale: 0.8, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 260, damping: 22 }}
        className="mx-auto flex size-14 items-center justify-center gap-[3px] rounded-2xl border border-line-2 bg-sunken"
      >
        {[0.35, 0.7, 1, 0.55, 0.4].map((height, index) => (
          <motion.span
            key={index}
            className="w-[3px] rounded-full bg-voice"
            style={{ height: `${height * 26}px` }}
            animate={{ scaleY: [0.7, 1, 0.7] }}
            transition={{
              duration: 1.8,
              repeat: Infinity,
              ease: "easeInOut",
              delay: index * 0.12,
            }}
          />
        ))}
      </motion.div>

      <h1 className="display mt-6 text-[26px] text-ink">
        Speak anywhere. clide types.
      </h1>
      <p className="mx-auto mt-3 max-w-[380px] text-[13.5px] leading-relaxed text-ink-2">
        Hold one shortcut, say what you mean, and clide transcribes it into
        whatever app you're using. Setup takes about a minute.
      </p>
    </div>
  );
}
