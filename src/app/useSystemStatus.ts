import { useCallback, useEffect, useState } from "react";
import * as commands from "@/lib/commands";
import { EVENTS, on } from "@/lib/events";
import type { SystemStatus } from "@/lib/types";

/**
 * Readiness, permissions, and settings in one value.
 *
 * Refetched whenever the backend says settings changed, and whenever the
 * window regains focus — permissions are granted in System Settings, outside
 * the app, so returning to clide is exactly when the answer may have changed.
 */
export function useSystemStatus() {
  const [status, setStatus] = useState<SystemStatus | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await commands.getSystemStatus());
    } catch (error) {
      console.error("could not read system status", error);
    }
  }, []);

  useEffect(() => {
    refresh();

    const subscription = on(EVENTS.settingsChanged, refresh);
    window.addEventListener("focus", refresh);

    return () => {
      subscription.then((unlisten) => unlisten());
      window.removeEventListener("focus", refresh);
    };
  }, [refresh]);

  return { status, refresh };
}
