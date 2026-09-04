import { ArrowUpRight, KeyRound } from "lucide-react";
import { Card, CardHeader } from "@/components/Card";
import { StatusDot } from "@/components/StatusDot";
import { Button } from "@/components/Button";
import type { SystemStatus } from "@/lib/types";

/**
 * Which engine is doing the transcribing, and whether it is usable.
 *
 * Shows `configured`, never anything derived from the key itself: the API key
 * never leaves the backend and no part of the UI has ever seen it.
 */
export function ProviderCard({
  status,
  onConfigure,
}: {
  status: SystemStatus;
  onConfigure: () => void;
}) {
  return (
    <Card index={1} className="col-span-12 flex flex-col p-4.5 lg:col-span-4">
      <CardHeader
        label="Engine"
        action={
          <Button size="sm" variant="ghost" onClick={onConfigure}>
            Change engine
            <ArrowUpRight size={12} />
          </Button>
        }
      />

      <p className="display mt-3 text-[19px]">{status.providerName}</p>
      <p className="text-[13px] text-ink-2">{status.modelName}</p>

      <div className="mt-auto flex items-center gap-2 pt-4 text-[12.5px] text-ink-2">
        <StatusDot tone={status.providerConfigured ? "ready" : "pending"} />
        {!status.providerNeedsKey ? (
          "No key needed — runs on this Mac"
        ) : status.providerConfigured ? (
          "Key stored on this Mac"
        ) : (
          <span className="inline-flex items-center gap-1.5 text-warn">
            <KeyRound size={12} />
            API key needed
          </span>
        )}
      </div>
    </Card>
  );
}
