import {
  Combobox,
  ComboboxChip,
  ComboboxChips,
  ComboboxChipsInput,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxItem,
  ComboboxList,
  useComboboxAnchor,
} from "@/components/ui/combobox";

export type MultiSelectOption = { value: string; label: string };

/** A dropdown of fixed options where several can be chosen; chosen ones show as chips. */
export function MultiSelect({
  id,
  options,
  value,
  onChange,
  placeholder = "Choose…",
  empty = "No options.",
  disabled,
}: {
  id?: string;
  options: MultiSelectOption[];
  value: string[];
  onChange: (value: string[]) => void;
  placeholder?: string;
  empty?: string;
  disabled?: boolean;
}) {
  const anchor = useComboboxAnchor();
  const labelOf = (item: string) => options.find((o) => o.value === item)?.label ?? item;
  return (
    <Combobox
      multiple
      autoHighlight
      items={options.map((o) => o.value)}
      value={value}
      onValueChange={(next: string[]) => onChange(next)}
      itemToStringLabel={labelOf}
      disabled={disabled}
    >
      <ComboboxChips ref={anchor}>
        {value.map((item) => (
          <ComboboxChip key={item}>{labelOf(item)}</ComboboxChip>
        ))}
        <ComboboxChipsInput id={id} placeholder={value.length ? "" : placeholder} />
      </ComboboxChips>
      <ComboboxContent anchor={anchor}>
        <ComboboxEmpty>{empty}</ComboboxEmpty>
        <ComboboxList>
          {(item: string) => (
            <ComboboxItem key={item} value={item}>
              {labelOf(item)}
            </ComboboxItem>
          )}
        </ComboboxList>
      </ComboboxContent>
    </Combobox>
  );
}
