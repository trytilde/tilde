import {
  Button,
  Form,
  Input,
  Label,
  NativeSelect,
  Page,
  mount,
  useSetup,
  useConnectionForm,
} from "@trytilde/connection-ui";
// Linking itself happens on Plaid's Hosted Link page (the shared redirect step); this page only
// collects a tenant's own Plaid keys, or starts a re-link of the same Item.
function Plaid() {
  const setup = useSetup();
  const step = setup.state?.step;
  const form = useConnectionForm({ environment: "production" });
  return (
    <Page setup={setup}>
      {step === "fields" && setup.state?.typeId === "own_keys" ? (
        <Form setup={setup} form={form}>
          <Label>
            Client ID
            <Input required {...form.register("client_id", { required: true })} />
          </Label>
          <Label>
            Secret
            <Input
              type="password"
              autoComplete="new-password"
              required
              {...form.register("secret", { required: true })}
            />
          </Label>
          <Label>
            Environment
            <NativeSelect {...form.register("environment", { required: true })}>
              <option value="production">Production</option>
              <option value="sandbox">Sandbox</option>
            </NativeSelect>
          </Label>
        </Form>
      ) : step === "fields" || step === "plaid_update" ? (
        <div className="space-y-4">
          <p>
            {step === "plaid_update"
              ? "Your institution needs you to sign in again. Re-link the same login on Plaid's page."
              : "Choose your institution and sign in on Plaid's page."}
          </p>
          <Button disabled={setup.busy} onClick={() => void setup.submit({})}>
            {step === "plaid_update" ? "Re-link with Plaid" : "Continue with Plaid"}
          </Button>
        </div>
      ) : null}
    </Page>
  );
}
mount(Plaid);
