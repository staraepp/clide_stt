import { useEffect, useRef, type RefObject } from "react";
import { EVENTS, on } from "@/lib/events";

/**
 * Subscribe to microphone level updates.
 *
 * Deliberately returns a ref rather than state: levels arrive about 30 times
 * a second and only the waveform canvas cares. Putting them in React state
 * would re-render the dashboard on every audio frame.
 */
export function useMicLevel(): RefObject<number> {
  const level = useRef(0);

  useEffect(() => {
    const subscription = on(EVENTS.dictationLevel, ({ level: value }) => {
      level.current = value;
    });
    return () => {
      subscription.then((unlisten) => unlisten());
    };
  }, []);

  return level;
}
