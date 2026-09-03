import { useEffect, useState } from "react";
import * as commands from "@/lib/commands";
import { EVENTS, on } from "@/lib/events";
import type { DictationState } from "@/lib/types";

/**
 * The dictation state, mirrored from Rust.
 *
 * The initial fetch matters: the HUD window is created hidden and may be shown
 * mid-transaction, so it has to be able to ask for the current state rather
 * than wait for the next event.
 */
export function useDictationState(): DictationState {
  const [state, setState] = useState<DictationState>({ kind: "idle" });

  useEffect(() => {
    let active = true;

    commands.getDictationState().then((current) => {
      if (active) setState(current);
    });

    const subscription = on(EVENTS.dictationState, (next) => {
      if (active) setState(next);
    });

    return () => {
      active = false;
      subscription.then((unlisten) => unlisten());
    };
  }, []);

  return state;
}
