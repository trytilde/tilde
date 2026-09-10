import {
  Button,
  Form,
  Input,
  Label,
  Page,
  mount,
  useSetup,
  useConnectionForm,
} from "@trytilde/connection-ui";
function Slack() {
  const setup = useSetup();
  const step = setup.state?.step;
  const form = useConnectionForm();
  return (
    <Page setup={setup} title="Connect Slack">
      <p>
        After authorizing, verify the Events API request URL in your Slack app’s settings. New apps
        include the event subscriptions in their manifest.
      </p>
      {step === "fields" ? (
        <div className="space-y-4">
          <p>Create a Slack app in your workspace, or connect an app you already own.</p>
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
          {step === "slack_create" ? (
            <>
              <Label>
                App name
                <Input required {...form.register("app_name", { required: true })} />
              </Label>
              <p>
                Get an app configuration refresh token from{" "}
                <a
                  href="https://api.slack.com/apps"
                  target="_blank"
                  rel="noreferrer"
                  className="underline"
                >
                  your Slack apps
                </a>
                . Tilde will rotate it to create your app.
              </p>
              <Label>
                Configuration refresh token
                <Input
                  type="password"
                  autoComplete="new-password"
                  required
                  {...form.register("configuration_refresh_token", { required: true })}
                />
              </Label>
            </>
          ) : (
            <>
              <p>In your Slack app’s OAuth settings, add this redirect URL:</p>
              <code className="block break-all text-sm">
                {new URL("/connections/callback", window.location.href).href}
              </code>
              <Label>
                Client ID
                <Input required {...form.register("client_id", { required: true })} />
              </Label>
              <Label>
                Client secret
                <Input
                  type="password"
                  required
                  {...form.register("client_secret", { required: true })}
                />
              </Label>
              <Label>
                Signing secret
                <Input
                  type="password"
                  required
                  {...form.register("signing_secret", { required: true })}
                />
              </Label>
            </>
          )}
        </Form>
      )}
    </Page>
  );
}
mount(Slack);
