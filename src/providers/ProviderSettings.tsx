import { useEffect, useState } from "react";
import { Check, ExternalLink, KeyRound, Loader2, Trash2 } from "lucide-react";

import { Button } from "@/components/Button";
import { StatusDot } from "@/components/StatusDot";
import * as commands from "@/lib/commands";
import type { ProviderDescriptor, ProviderStatus } from "@/lib/types";
import { cn } from "@/lib/cn";

type Phase = "idle" | "saving" | "saved" | "error";

/**
 * Provider configuration.
 *
 * The key is typed here and goes straight to Rust, which validates it against
 * the provider and stores it on disk. It is never persisted in frontend state,
 * never echoed back, and the input is cleared on success.
 */
export function ProviderSettings({ onChange }: { onChange: () => void }) {
  const [providers, setProviders] = useState<ProviderDescriptor[]>([]);
  const [statuses, setStatuses] = useState<ProviderStatus[]>([]);

  const load = async () => {
    const [descriptors, status] = await Promise.all([
      commands.listProviders(),
      commands.getProviderStatus(),
    ]);
    setProviders(descriptors);
    setStatuses(status);
  };

  useEffect(() => {
    load();
  }, []);

  return (
    <div className="flex flex-col gap-3">
      {providers.map((provider) => {
        const status = statuses.find((entry) => entry.id === provider.id);
        return (
          <ProviderRow
            key={provider.id}
            provider={provider}
            status={status}
            onChanged={() => {
              load();
              onChange();
            }}
          />
        );
      })}
    </div>
  );
}

function ProviderRow({
  provider,
  status,
  onChanged,
}: {
  provider: ProviderDescriptor;
  status?: ProviderStatus;
  onChanged: () => void;
}) {
  const [key, setKey] = useState("");
  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState<string | null>(null);

  const configured = status?.configured ?? false;
  const needsKey = provider.credential.kind === "apiKey";

  const save = async () => {
    setPhase("saving");
    setMessage(null);
    try {
      await commands.saveProviderKey(provider.id, key);
      setKey("");
      setPhase("saved");
      onChanged();
    } catch (error) {
      setPhase("error");
      setMessage(commands.errorMessage(error));
    }
  };

  const remove = async () => {
    try {
      await commands.removeProviderKey(provider.id);
      setPhase("idle");
      setMessage(null);
      onChanged();
    } catch (error) {
      setMessage(commands.errorMessage(error));
    }
  };

  const test = async () => {
    setPhase("saving");
    setMessage(null);
    try {
      await commands.validateProvider(provider.id);
      setPhase("saved");
      setMessage("Key works.");
    } catch (error) {
      setPhase("error");
      setMessage(commands.errorMessage(error));
    }
  };

  return (
    <div className="card p-5">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <StatusDot tone={configured ? "ready" : "pending"} />
          <span className="display text-[15px] text-ink">
            {provider.name}
          </span>
          {status?.selected && (
            <span className="rounded-md bg-voice-tint px-1.5 py-0.5 text-[10.5px] font-medium text-voice-deep">
              In use
            </span>
          )}
        </div>

        <CapabilityChips provider={provider} />
      </div>

      <div className="mt-4 flex flex-wrap gap-1.5">
        {provider.models.map((model) => {
          const selected = status?.modelId === model.id;
          return (
            <button
              key={model.id}
              type="button"
              title={model.description}
              onClick={async () => {
                await commands.selectProvider(provider.id, model.id);
                onChanged();
              }}
              className={cn(
                "no-drag rounded-lg border px-2.5 py-1.5 text-[12px] transition-colors",
                selected
                  ? "border-voice bg-voice-tint text-ink"
                  : "border-line text-ink-2 hover:border-line-2 hover:text-ink",
              )}
            >
              {model.name}
            </button>
          );
        })}
      </div>

      {needsKey && (
        <div className="mt-4">
          {configured ? (
            <div className="flex flex-wrap items-center gap-2">
              <span className="inline-flex items-center gap-1.5 text-[12.5px] text-ink-2">
                <KeyRound size={13} />
                Key stored on this Mac
              </span>
              <div className="ml-auto flex gap-1.5">
                <Button size="sm" onClick={test} disabled={phase === "saving"}>
                  {phase === "saving" ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    "Test"
                  )}
                </Button>
                <Button size="sm" variant="danger" icon={<Trash2 size={13} />} onClick={remove}>
                  Remove
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex flex-wrap items-center gap-2">
              <input
                type="password"
                value={key}
                onChange={(event) => setKey(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && key.trim()) save();
                }}
                placeholder={
                  provider.credential.kind === "apiKey" &&
                  provider.credential.expectedPrefix
                    ? `${provider.credential.expectedPrefix}…`
                    : "API key"
                }
                autoComplete="off"
                spellCheck={false}
                className={cn(
                  "no-drag h-9 flex-1 min-w-[200px] rounded-[10px] border border-line bg-sunken px-3",
                  "font-mono text-[12.5px] text-ink placeholder:text-ink-2/60",
                  "outline-none transition-colors focus:border-voice",
                )}
              />
              <Button
                variant="primary"
                onClick={save}
                disabled={!key.trim() || phase === "saving"}
              >
                {phase === "saving" ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  "Save and verify"
                )}
              </Button>
            </div>
          )}

          {message && (
            <p
              className={cn(
                "mt-2.5 inline-flex items-center gap-1.5 text-[12px]",
                phase === "error" ? "text-stop" : "text-ok",
              )}
            >
              {phase === "saved" && <Check size={12} />}
              {message}
            </p>
          )}

          {!configured && provider.credential.kind === "apiKey" && (
            <button
              type="button"
              onClick={() =>
                provider.credential.kind === "apiKey" &&
                commands
                  .copyText(provider.credential.helpUrl)
                  .then(() => setMessage("Link copied to your clipboard."))
              }
              className="no-drag mt-2.5 inline-flex items-center gap-1 text-[12px] text-ink-2 hover:text-voice-deep"
            >
              Where do I get a key?
              <ExternalLink size={11} />
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * Capabilities, not provider names. This is the surface that lets the UI stay
 * honest as backends with different feature sets are added.
 */
function CapabilityChips({ provider }: { provider: ProviderDescriptor }) {
  const chips = [
    provider.capabilities.local ? "Local" : "Cloud",
    provider.capabilities.streaming ? "Streaming" : null,
    provider.capabilities.languageDetection ? "Auto language" : null,
    provider.capabilities.wordTimestamps ? "Word timing" : null,
  ].filter(Boolean) as string[];

  return (
    <div className="flex flex-wrap justify-end gap-1">
      {chips.map((chip) => (
        <span
          key={chip}
          className="rounded-md border border-line px-1.5 py-0.5 text-[10.5px] text-ink-2"
        >
          {chip}
        </span>
      ))}
    </div>
  );
}
