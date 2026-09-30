import { Switch as BaseSwitch } from "@base-ui/react/switch";

export function Switch({ checked, onCheckedChange, disabled, label }: {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  label: string;
}) {
  return <BaseSwitch.Root className="ui-switch" checked={checked} onCheckedChange={onCheckedChange} disabled={disabled} aria-label={label}>
    <BaseSwitch.Thumb className="ui-switch-thumb" />
  </BaseSwitch.Root>;
}
