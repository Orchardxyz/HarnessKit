import { clsx } from "clsx";
import { Check, ChevronDown } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import { agentDisplayName } from "@/lib/types";
import { isWeb as web } from "@/lib/web-select";

/** Per-agent border colors for the active filter trigger state. */
const AGENT_FILTER_BORDERS: Record<string, string> = {
  claude: "border-agent-claude",
  codex: "border-agent-codex",
  gemini: "border-agent-gemini",
  cursor: "border-agent-cursor",
  antigravity: "border-agent-antigravity",
  copilot: "border-agent-copilot",
  windsurf: "border-agent-windsurf",
  opencode: "border-agent-opencode",
};

interface AgentOption {
  name: string | null;
  label: string;
}

interface AgentFilterProps {
  agentFilter: string | null;
  enabledAgents: { name: string }[];
  onChange: (agent: string | null) => void;
  ariaLabel: string;
  allAgentsLabel: string;
}

export function AgentFilter({
  agentFilter,
  enabledAgents,
  onChange,
  ariaLabel,
  allAgentsLabel,
}: AgentFilterProps) {
  const options = useMemo<AgentOption[]>(
    () => [
      { name: null, label: allAgentsLabel },
      ...enabledAgents.map((agent) => ({
        name: agent.name,
        label: agentDisplayName(agent.name),
      })),
    ],
    [allAgentsLabel, enabledAgents],
  );
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const selectedIndex = options.findIndex((o) => o.name === agentFilter);
  const [activeIndex, setActiveIndex] = useState(
    selectedIndex >= 0 ? selectedIndex : 0,
  );

  useEffect(() => {
    if (!open) {
      setActiveIndex(selectedIndex >= 0 ? selectedIndex : 0);
    }
  }, [open, selectedIndex]);

  useEffect(() => {
    if (!open) return;
    const onMouseDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setOpen(false);
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setActiveIndex((i) => Math.min(i + 1, options.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setActiveIndex((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        const option = options[activeIndex];
        if (!option) return;
        onChange(option.name);
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [activeIndex, onChange, open, options]);

  const selectedLabel =
    options.find((o) => o.name === agentFilter)?.label ?? allAgentsLabel;
  const selectedAgent = agentFilter ?? null;

  return (
    <div ref={rootRef} className="relative shrink-0">
      <button
        type="button"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        onKeyDown={(e) => {
          if (
            e.key === "ArrowDown" ||
            e.key === "ArrowUp" ||
            e.key === "Enter" ||
            e.key === " "
          ) {
            e.preventDefault();
            setOpen(true);
          }
        }}
        className={clsx(
          "flex min-w-36 items-center gap-2 border bg-card px-3 text-xs text-foreground transition-colors focus:outline-none",
          web ? "h-[26px] rounded-[6px]" : "rounded-lg py-1.5",
          agentFilter && AGENT_FILTER_BORDERS[agentFilter]
            ? AGENT_FILTER_BORDERS[agentFilter]
            : "border-border focus:border-ring",
        )}
      >
        {selectedAgent && <AgentMascot name={selectedAgent} size={16} />}
        <span className="truncate">{selectedLabel}</span>
        <ChevronDown
          size={14}
          className="ml-auto shrink-0 text-muted-foreground"
        />
      </button>
      {open && (
        <div
          role="listbox"
          aria-label={ariaLabel}
          className="absolute right-0 top-full z-50 mt-1 min-w-52 overflow-hidden rounded-xl border border-border/60 bg-background p-1 shadow-sm"
        >
          {options.map((option, index) => {
            const selected = option.name === agentFilter;
            const active = index === activeIndex;
            return (
              <button
                key={option.name ?? "all"}
                type="button"
                role="option"
                aria-selected={selected}
                data-active={active ? "true" : undefined}
                onMouseEnter={() => setActiveIndex(index)}
                onClick={() => {
                  onChange(option.name);
                  setOpen(false);
                }}
                className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm text-foreground hover:bg-accent/60 data-[active=true]:bg-accent"
              >
                {option.name && <AgentMascot name={option.name} size={16} />}
                <span className="flex-1 text-left">{option.label}</span>
                {selected && (
                  <Check size={12} className="shrink-0 text-foreground" />
                )}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
