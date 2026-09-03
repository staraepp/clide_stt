import { useCallback, useEffect, useState } from "react";
import * as commands from "@/lib/commands";
import { EVENTS, on } from "@/lib/events";
import type { HistoryQuery, Transcript } from "@/lib/types";

/**
 * Transcript history for a given query.
 *
 * Search runs in SQLite's FTS index rather than in JavaScript, so the query
 * goes to Rust on every keystroke instead of filtering a cached array.
 */
export function useHistory(query: HistoryQuery) {
  const [transcripts, setTranscripts] = useState<Transcript[]>([]);
  const [loading, setLoading] = useState(true);

  const key = JSON.stringify(query);

  const refresh = useCallback(async () => {
    try {
      setTranscripts(await commands.getHistory(JSON.parse(key)));
    } catch (error) {
      console.error("could not read history", error);
    } finally {
      setLoading(false);
    }
  }, [key]);

  useEffect(() => {
    refresh();
    const subscription = on(EVENTS.historyChanged, refresh);
    return () => {
      subscription.then((unlisten) => unlisten());
    };
  }, [refresh]);

  return { transcripts, loading, refresh };
}
