import { DictationCard } from "./cards/DictationCard";
import { ProviderCard } from "./cards/ProviderCard";
import { ModeCard } from "./cards/ModeCard";
import { RecentCard } from "./cards/RecentCard";
import { UsageCard } from "./cards/UsageCard";
import { SystemCard } from "./cards/SystemCard";
import { useDictationState } from "@/dictation/useDictationState";
import { useMicLevel } from "@/dictation/useMicLevel";
import type { SystemStatus } from "@/lib/types";

/**
 * The bento dashboard.
 *
 * Fixed layout by design: v0.1 shows what a customisable grid will look like
 * without paying for drag, resize, and persistence before dictation is solid.
 */
export function Dashboard({
  status,
  refresh,
  onNavigate,
}: {
  status: SystemStatus;
  refresh: () => void;
  onNavigate: (view: "models" | "history" | "settings") => void;
}) {
  const state = useDictationState();
  const level = useMicLevel();

  return (
    <div className="grid auto-rows-min grid-cols-12 gap-3 py-3">
      <DictationCard state={state} status={status} levelRef={level} />
      <ProviderCard status={status} onConfigure={() => onNavigate("models")} />
      <ModeCard mode={status.settings.mode} onChange={refresh} />
      <RecentCard onOpenHistory={() => onNavigate("history")} />

      <SystemCard status={status} onRefresh={refresh} />

      <UsageCard />
    </div>
  );
}
