import { Select } from "@base-ui/react/select";
import { CaretDown, Check } from "@phosphor-icons/react";
import { AgentBrandIcon } from "../../lib/brand-icons";
import { sessionAgentName, sessionAgents } from "../../lib/sessions";

export function SessionAgentLabel({ adapter, name }: { adapter: string; name?: string }) {
  return <span className="session-agent-label"><AgentBrandIcon agent={adapter} /><span>{name ?? sessionAgentName(adapter)}</span></span>;
}

export function SessionAgentSelect({ value, onChange, label, allLabel }: {
  value: string; onChange: (value: string) => void; label: string; allLabel?: string;
}) {
  const items = Object.entries(sessionAgents).map(([value, label]) => ({ value, label }));
  if (allLabel) items.unshift({ value: "", label: allLabel });
  return <Select.Root value={value} onValueChange={next => { if (next !== null) onChange(next); }} items={items}>
    <Select.Trigger className="session-agent-select" aria-label={label}>
      <Select.Value>{value ? <SessionAgentLabel adapter={value} /> : allLabel}</Select.Value>
      <Select.Icon><CaretDown aria-hidden="true" /></Select.Icon>
    </Select.Trigger>
    <Select.Portal><Select.Positioner className="session-agent-positioner" sideOffset={6} alignItemWithTrigger={false}>
      <Select.Popup className="session-agent-popup"><Select.List>
        {items.map(item => <Select.Item key={item.value} value={item.value} className="session-agent-option">
          <Select.ItemText>{item.value ? <SessionAgentLabel adapter={item.value} /> : item.label}</Select.ItemText>
          <Select.ItemIndicator><Check aria-hidden="true" /></Select.ItemIndicator>
        </Select.Item>)}
      </Select.List></Select.Popup>
    </Select.Positioner></Select.Portal>
  </Select.Root>;
}
