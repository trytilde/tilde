import {
  Button,
  Form,
  Input,
  Label,
  NativeSelect,
  Textarea,
  Page,
  mount,
  useSetup,
  useConnectionForm,
} from "@trytilde/connection-ui";
function Github() {
  const setup = useSetup();
  const step = setup.state?.step;
  const form = useConnectionForm({ owner_type: "user" });
  const owner = form.watch("owner_type") ?? "user";
  return (
    <Page setup={setup}>
      {step === "fields" ? (
        <div className="space-y-4">
          <p>Create a GitHub App or connect an existing installation.</p>
          <div className="flex gap-3">
            <Button
              disabled={setup.busy}
              onClick={() => void setup.submit({ setup_path: "create" })}
            >
              Create an app
            </Button>
            <Button
              variant="outline"
              disabled={setup.busy}
              onClick={() => void setup.submit({ setup_path: "existing" })}
            >
              Use existing app
            </Button>
          </div>
        </div>
      ) : (
        <Form setup={setup} form={form}>
          {step === "github_create" ? (
            <>
              <Label>
                App name
                <Input required {...form.register("app_name", { required: true })} />
              </Label>
              <Label>
                App owner
                <NativeSelect {...form.register("owner_type", { required: true })}>
                  <option value="user">Personal account</option>
                  <option value="organization">Organization</option>
                </NativeSelect>
              </Label>
              {owner === "organization" && (
                <Label>
                  Organization
                  <Input
                    required
                    pattern="[A-Za-z0-9-]+"
                    {...form.register("account", { required: true })}
                  />
                </Label>
              )}
            </>
          ) : (
            <>
              <Label>
                App ID
                <Input type="number" required {...form.register("app_id", { required: true })} />
              </Label>
              <Label>
                Private key (PEM)
                <Textarea
                  autoComplete="off"
                  spellCheck={false}
                  required
                  {...form.register("private_key", { required: true })}
                />
              </Label>
              <Label>
                Installation ID
                <Input
                  type="number"
                  required
                  {...form.register("installation_id", { required: true })}
                />
              </Label>
              <Label>
                Webhook secret
                <Input
                  type="password"
                  required
                  {...form.register("webhook_secret", { required: true })}
                />
              </Label>
            </>
          )}
        </Form>
      )}
    </Page>
  );
}
mount(Github);
