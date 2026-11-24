// Drives the built provider pages (`task build:connection-ui`) in headless Chromium through the
// production iframe nesting: standalone (the broker page at the top) and inside the connection
// setup dialog's sandboxed iframe. A stand-in broker answers the page's MessagePort calls and
// walks GitHub's setup steps, so this checks the page, not the setup service. It also hands the
// page a theme with the port and a different one later, and reads what the page then paints.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const themes = [
  { scheme: "dark", tokens: { canvas: "#211714", accent: "#89c0ff", font: "Courier New" } },
  { scheme: "light", tokens: { accent: "#0038ae", radius: "0px" } },
];
/** What the page paints: its background and font, and the primary button's fill, text and corners. */
const painted = (button) => {
  const rgb = (colour) => {
    const context = new OffscreenCanvas(1, 1).getContext("2d");
    context.fillStyle = colour;
    context.fillRect(0, 0, 1, 1);
    return context.getImageData(0, 0, 1, 1).data.slice(0, 3).join(" ");
  };
  const body = getComputedStyle(document.body);
  const style = getComputedStyle(button);
  return {
    page: rgb(body.backgroundColor),
    font: body.fontFamily,
    heading: getComputedStyle(document.querySelector("h1")).fontFamily,
    fill: rgb(style.backgroundColor),
    text: rgb(style.color),
    radius: style.borderTopLeftRadius,
    dark: document.documentElement.classList.contains("dark"),
  };
};
const dist = fileURLToPath(new URL("../provider-dist/", import.meta.url));
// The sandbox attributes match connection-setup-dialog.tsx and -connection-setup-host.tsx.
const pages = {
  "/dialog.html": `<!doctype html><iframe id="dialog" src="/host.html" style="width:600px;height:1000px"
    sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox"></iframe>`,
  "/host.html": `<!doctype html><iframe id="provider" src="/connections/ui/github/ui.html" style="width:100%;height:1000px"
    sandbox="allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-top-navigation-by-user-activation"></iframe>
    <script>
      window.calls = [];
      let state = {
        providerName: "GitHub", connectionName: "GitHub", accountNameLabel: "GitHub account name",
        webhookUrl: "https://trytilde.ai/connections/webhooks/conn", setupInstructions: [],
        actionId: "action", authDriver: 5, step: "fields", action: { case: "form", value: {} },
      };
      function answer(call) {
        window.calls.push({ method: call.method, fields: call.fields });
        if (call.method !== "executeProviderAction") return state;
        if (state.step === "fields") state = { ...state, step: "github_" + call.fields.setup_path };
        else if (state.step === "github_create")
          state = { ...state, step: "github_manifest", action: { case: "formPost", value: {
            url: "https://github.com/organizations/" + call.fields.account + "/settings/apps/new",
            fields: [{ name: "manifest", value: "{}" }] } } };
        return state;
      }
      addEventListener("message", (event) => {
        const frame = document.getElementById("provider");
        if (event.source !== frame.contentWindow || event.data?.type !== "tilde.setup.ready") return;
        const channel = new MessageChannel();
        channel.port1.onmessage = ({ data }) => channel.port1.postMessage({ id: data.id, state: answer(data) });
        window.setTheme = (theme) => channel.port1.postMessage({ theme });
        frame.contentWindow.postMessage({ type: "tilde.setup.port", theme: ${JSON.stringify(themes[0])} }, "*", [channel.port2]);
      });
    </script>`,
};
const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css" };
const server = createServer(async (request, response) => {
  const path = new URL(request.url, "http://localhost").pathname;
  // The provider frame has an opaque origin, so its module scripts are cross-origin requests.
  response.setHeader("Access-Control-Allow-Origin", "*");
  if (pages[path]) return response.writeHead(200, { "Content-Type": "text/html" }).end(pages[path]);
  const file = normalize(path.replace(/^\/connections\/ui\//, ""));
  if (!path.startsWith("/connections/ui/") || file.startsWith(".."))
    return response.writeHead(404).end();
  try {
    const body = await readFile(join(dist, file));
    response.writeHead(200, { "Content-Type": types[extname(file)] ?? "application/octet-stream" });
    response.end(body);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch();
try {
  for (const top of ["host", "dialog"]) {
    const page = await browser.newPage();
    const errors = [];
    page.on("console", (message) => message.type() === "error" && errors.push(message.text()));
    await page.goto(`${origin}/${top}.html`);
    const host = top === "dialog" ? page.frameLocator("#dialog") : page;
    const frame = host.frameLocator("#provider");
    await frame.getByLabel("GitHub account name").fill("acmeai");
    await frame.getByRole("button", { name: "Create an app" }).click();
    await frame.getByLabel("App name").fill("acmeai");
    await frame.getByLabel("App owner").selectOption("organization");
    const organization = frame.getByRole("textbox", { name: "Organization" });
    await organization.fill("acme engineering");
    const hostFrame = top === "dialog" ? page.frame({ url: /host\.html/ }) : page.mainFrame();
    const submit = frame.getByRole("button", { name: "Continue" });
    // The theme that came with the port: dark, the host's canvas and font, dark text on a light accent.
    assert.deepEqual(await submit.evaluate(painted), {
      page: "33 23 20",
      font: '"Courier New"',
      heading: '"Courier New"',
      fill: "137 192 255",
      text: "0 0 0",
      radius: "8px",
      dark: true,
    });
    // A later theme replaces it: tokens it leaves out go back to the page's own light values.
    await hostFrame.evaluate((theme) => window.setTheme(theme), themes[1]);
    await frame.locator("html:not(.dark)").waitFor();
    // The button's colours transition; read them once they have arrived.
    let light = await submit.evaluate(painted);
    for (let tries = 0; light.fill !== "0 56 174" && tries < 40; tries++) {
      await page.waitForTimeout(50);
      light = await submit.evaluate(painted);
    }
    assert.deepEqual(
      {
        ...light,
        font: light.font.includes("Geist"),
        heading: light.heading.includes("Bricolage"),
      },
      {
        page: "255 255 255",
        font: true,
        heading: true,
        fill: "0 56 174",
        text: "255 255 255",
        radius: "0px",
        dark: false,
      },
    );
    await frame.getByRole("button", { name: "Continue" }).click();
    await frame
      .getByRole("alert")
      .getByText(/Organization/)
      .waitFor();
    await organization.fill("acme-engineering");
    await frame.getByRole("button", { name: "Continue" }).click();
    const post = frame.getByRole("button", { name: "Create app with provider" });
    await post.waitFor();
    assert.equal(
      await post.evaluate((button) => button.form.action),
      "https://github.com/organizations/acme-engineering/settings/apps/new",
      top,
    );
    const calls = await hostFrame.evaluate(() => window.calls);
    assert.deepEqual(calls.at(-1).fields, {
      app_name: "acmeai",
      owner_type: "organization",
      account: "acme-engineering",
    });
    assert.deepEqual(errors, [], `${top} console errors`);
    console.log(`ok - GitHub App create continues to the manifest post (${top})`);
    await page.close();
  }
} finally {
  await browser.close();
  server.close();
}
