import { useEffect, useState } from "react";
import { Controller, useFormContext } from "react-hook-form";
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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

/** The subset of JSON Schema the form renders. */
export type JsonSchema = {
  type?: string;
  title?: string;
  description?: string;
  default?: unknown;
  enum?: string[];
  properties?: Record<string, JsonSchema>;
  items?: JsonSchema;
  /** A setup query whose results are the choices of an array of strings. */
  "x-tilde-options"?: string;
};
export type Option = { value: string; label: string };
export type LoadOptions = (query: string) => Promise<Option[]>;

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

/** `value` with every property it lacks filled from the schema's defaults. */
export function schemaDefaults(schema: JsonSchema, value?: unknown): unknown {
  if (schema.type === "object") {
    const given = isObject(value) ? value : isObject(schema.default) ? schema.default : {};
    return Object.fromEntries(
      Object.entries(schema.properties ?? {}).map(([key, property]) => [
        key,
        schemaDefaults(property, given[key]),
      ]),
    );
  }
  if (value !== undefined) return value;
  if (schema.default !== undefined) return schema.default;
  if (schema.type === "array") return [];
  if (schema.type === "boolean") return false;
  return "";
}

/**
 * Fields for a JSON Schema object under `name` in the surrounding React Hook Form: nested objects,
 * booleans, strings (enums as a select) and arrays of strings.
 */
export function SchemaForm({
  schema,
  name,
  loadOptions,
  disabled,
}: {
  schema: JsonSchema;
  name: string;
  loadOptions?: LoadOptions;
  disabled?: boolean;
}) {
  return (
    <div className="space-y-4">
      {Object.entries(schema.properties ?? {}).map(([key, property]) => (
        <SchemaField
          key={key}
          schema={property}
          name={`${name}.${key}`}
          label={property.title ?? key}
          loadOptions={loadOptions}
          disabled={disabled}
        />
      ))}
    </div>
  );
}

function SchemaField({
  schema,
  name,
  label,
  loadOptions,
  disabled,
}: {
  schema: JsonSchema;
  name: string;
  label: string;
  loadOptions?: LoadOptions;
  disabled?: boolean;
}) {
  const { control, register } = useFormContext();
  const id = `schema-${name.replaceAll(".", "-")}`;
  const help = schema.description && (
    <p id={`${id}-help`} className="text-xs text-muted-foreground">
      {schema.description}
    </p>
  );
  if (schema.type === "object")
    return (
      <fieldset className="space-y-4 rounded-lg border p-4">
        <legend className="px-1 text-sm font-medium">{label}</legend>
        {help}
        <SchemaForm schema={schema} name={name} loadOptions={loadOptions} disabled={disabled} />
      </fieldset>
    );
  if (schema.type === "boolean")
    return (
      <Controller
        control={control}
        name={name}
        render={({ field }) => (
          <div className="flex items-start justify-between gap-4">
            <div className="space-y-1">
              <Label htmlFor={id}>{label}</Label>
              {help}
            </div>
            <Switch
              id={id}
              checked={!!field.value}
              onCheckedChange={field.onChange}
              disabled={disabled}
              aria-describedby={help ? `${id}-help` : undefined}
            />
          </div>
        )}
      />
    );
  if (schema.type === "array")
    return (
      <div className="space-y-2">
        <Label htmlFor={id}>{label}</Label>
        <Controller
          control={control}
          name={name}
          render={({ field }) => (
            <StringList
              id={id}
              value={Array.isArray(field.value) ? (field.value as string[]) : []}
              onChange={field.onChange}
              query={schema["x-tilde-options"]}
              loadOptions={loadOptions}
              disabled={disabled}
            />
          )}
        />
        {help}
      </div>
    );
  if (schema.enum)
    return (
      <div className="space-y-2">
        <Label htmlFor={id}>{label}</Label>
        <Controller
          control={control}
          name={name}
          render={({ field }) => (
            <Select
              value={field.value as string}
              onValueChange={field.onChange}
              disabled={disabled}
              items={schema.enum!.map((value) => ({ value, label: value }))}
            >
              <SelectTrigger id={id} className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {schema.enum!.map((value) => (
                  <SelectItem key={value} value={value}>
                    {value}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        />
        {help}
      </div>
    );
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        disabled={disabled}
        aria-describedby={help ? `${id}-help` : undefined}
        {...register(name)}
      />
      {help}
    </div>
  );
}

/** Strings as chips: picked from the query's options when it has one, or typed and Enter. */
function StringList({
  id,
  value,
  onChange,
  query,
  loadOptions,
  disabled,
}: {
  id: string;
  value: string[];
  onChange: (value: string[]) => void;
  query?: string;
  loadOptions?: LoadOptions;
  disabled?: boolean;
}) {
  const anchor = useComboboxAnchor();
  const [input, setInput] = useState("");
  const [options, setOptions] = useState<Option[]>([]);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!query || !loadOptions) return;
    let current = true;
    loadOptions(query).then(
      (found) => current && setOptions(found),
      () => current && setFailed(true),
    );
    return () => {
      current = false;
    };
  }, [query, loadOptions]);
  const typed = input.trim();
  const items = options.map((option) => option.value);
  // Free text is always allowed: what is typed is offered alongside the matching options.
  if (typed && !items.includes(typed)) items.push(typed);
  const labelOf = (item: string) => options.find((option) => option.value === item)?.label ?? item;
  return (
    <>
      <Combobox
        multiple
        autoHighlight
        items={items}
        value={value}
        onValueChange={(next: string[]) => {
          onChange(next);
          setInput("");
        }}
        inputValue={input}
        onInputValueChange={setInput}
        itemToStringLabel={labelOf}
        disabled={disabled}
      >
        <ComboboxChips ref={anchor}>
          {value.map((item) => (
            <ComboboxChip key={item}>{labelOf(item)}</ComboboxChip>
          ))}
          <ComboboxChipsInput
            id={id}
            placeholder={query ? "Search or type, then Enter" : "Type, then Enter"}
          />
        </ComboboxChips>
        <ComboboxContent anchor={anchor}>
          <ComboboxEmpty>
            {query && !failed ? "No matches. Type a value." : "Type a value."}
          </ComboboxEmpty>
          <ComboboxList>
            {(item: string) => (
              <ComboboxItem key={item} value={item}>
                {labelOf(item)}
              </ComboboxItem>
            )}
          </ComboboxList>
        </ComboboxContent>
      </Combobox>
      {failed && (
        <p className="text-xs text-muted-foreground">
          Unable to load choices; type values instead.
        </p>
      )}
    </>
  );
}
