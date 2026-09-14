import { useEffect, useMemo } from "react";
import { AuthDriver, type Brokering } from "@trytilde/sdk/connection-setup";
import { Page, Form, useSetup, useConnectionForm, type Setup } from "./setup.js";
import { Input } from "./components/input.js";
import { Label, Textarea, NativeSelect } from "./components/controls.js";

type Property = {
  type: "string" | "boolean" | "integer" | "number";
  title?: string;
  description?: string;
  writeOnly?: boolean;
  enum?: Array<string | boolean | number>;
  default?: string | boolean | number;
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  minimum?: number;
  maximum?: number;
  format?: string;
  contentMediaType?: string;
};
type Schema = {
  type: "object";
  properties: Record<string, Property>;
  required?: string[];
  description?: string;
};
function encode(property: Property, value: string | boolean | number) {
  if (!property.enum) return String(value);
  if (property.type === "string") return JSON.stringify(String(value));
  try {
    return JSON.stringify(JSON.parse(String(value)));
  } catch {
    return "";
  }
}
function decode(property: Property, value: string) {
  return property.enum ? String(JSON.parse(value)) : value;
}
/** One standard renderer for the supported JSON Schema subset, with no provider-specific branches. */
export function CredentialForm({ setup, state }: { setup: Setup; state: Brokering }) {
  const schema = useMemo(
    () => JSON.parse(state.inputSchemaJson!) as Schema,
    [state.inputSchemaJson],
  );
  const entries = Object.entries(schema.properties);
  const required = new Set(schema.required ?? []);
  const defaults = Object.fromEntries(
    entries.flatMap(([, property], index) =>
      property.default === undefined
        ? []
        : [[`field_${index}`, encode(property, property.default)]],
    ),
  );
  const form = useConnectionForm(defaults);
  useEffect(() => {
    for (const [index, [key, property]] of entries.entries()) {
      const draft = state.draft.find((field) => field.key === key);
      if (draft) form.setValue(`field_${index}`, encode(property, draft.value));
    }
  }, [state.actionId]);
  function payload(values: Record<string, string>) {
    return Object.fromEntries(
      entries.flatMap(([key, property], index) => {
        const value = values[`field_${index}`];
        if (value === undefined || (value === "" && !required.has(key))) return [];
        return [[key, decode(property, value)]];
      }),
    );
  }
  function validate(property: Property, key: string, value: string) {
    if (value === undefined) return required.has(key) ? "This field is required" : true;
    if (value === "")
      return required.has(key) &&
        (property.type !== "string" || property.enum || !!property.minLength)
        ? "This field is required"
        : true;
    const decoded = decode(property, value);
    if (property.type === "string") {
      if (property.minLength !== undefined && Array.from(decoded).length < property.minLength)
        return `Use at least ${property.minLength} characters`;
      if (property.maxLength !== undefined && Array.from(decoded).length > property.maxLength)
        return `Use at most ${property.maxLength} characters`;
      if (property.pattern && !new RegExp(property.pattern, "u").test(decoded))
        return "This value does not match the required format";
    } else if (property.type !== "boolean") {
      const number = Number(decoded);
      if (!Number.isFinite(number) || (property.type === "integer" && !Number.isInteger(number)))
        return "Enter a valid number";
      if (property.minimum !== undefined && number < property.minimum)
        return `Minimum: ${property.minimum}`;
      if (property.maximum !== undefined && number > property.maximum)
        return `Maximum: ${property.maximum}`;
    }
    return true;
  }
  return (
    <Form setup={setup} form={form} onSubmit={(values) => setup.submit(payload(values))}>
      {state.authDriver === AuthDriver.OAUTH_CODE && (
        <p className="text-sm">
          OAuth redirect URL:{" "}
          <code>{new URL("/connections/callback", window.location.href).href}</code>
        </p>
      )}
      {entries.map(([key, property], index) => {
        const name = `field_${index}`;
        const registration = form.register(name, {
          validate: (value) => validate(property, key, value),
        });
        const error = form.formState.errors[name]?.message;
        return (
          <div key={key} className="space-y-1">
            <Label>
              {property.title ?? key}
              {property.enum ? (
                <NativeSelect {...registration} aria-invalid={!!error}>
                  <option value="">Select…</option>
                  {property.enum.map((value) => (
                    <option key={JSON.stringify(value)} value={JSON.stringify(value)}>
                      {String(value)}
                    </option>
                  ))}
                </NativeSelect>
              ) : property.type === "boolean" ? (
                <NativeSelect {...registration} aria-invalid={!!error}>
                  <option value="">Select…</option>
                  <option value="true">Yes</option>
                  <option value="false">No</option>
                </NativeSelect>
              ) : property.contentMediaType ? (
                <Textarea
                  {...registration}
                  aria-invalid={!!error}
                  autoComplete="off"
                  spellCheck={false}
                />
              ) : (
                <Input
                  {...registration}
                  aria-invalid={!!error}
                  type={property.writeOnly ? "password" : "text"}
                  inputMode={
                    property.type === "integer"
                      ? "numeric"
                      : property.type === "number"
                        ? "decimal"
                        : property.format === "email"
                          ? "email"
                          : property.format === "uri"
                            ? "url"
                            : undefined
                  }
                  autoComplete={property.writeOnly ? "new-password" : "off"}
                />
              )}
            </Label>
            {property.description && (
              <p className="text-xs text-muted-foreground">{property.description}</p>
            )}
            {error && (
              <p role="alert" className="text-sm text-destructive">
                {error}
              </p>
            )}
          </div>
        );
      })}
    </Form>
  );
}
export function StandardSetup() {
  const setup = useSetup();
  return (
    <Page setup={setup}>
      {setup.state?.inputSchemaJson && (
        <CredentialForm key={setup.state.inputSchemaJson} setup={setup} state={setup.state} />
      )}
    </Page>
  );
}
