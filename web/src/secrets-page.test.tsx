import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { create } from "@bufbuild/protobuf";
import { SecretSchema } from "@trytilde/contracts/tilde/management/v1/secrets_pb.js";
import { SecretsPage } from "@/components/secrets-page";

const rpc = vi.hoisted(() => ({
  secrets: { listSecrets: vi.fn(), setSecret: vi.fn(), deleteSecret: vi.fn() },
}));
vi.mock("@/client", () => rpc);
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it("the ?add deep link opens a row with the name filled in and the value focused", async () => {
  rpc.secrets.listSecrets.mockResolvedValue({
    secrets: [
      create(SecretSchema, {
        id: "s1",
        name: "GITHUB_TOKEN",
        description: "Bot token",
        blueprints: ["build-box"],
        blueprintCount: 2,
      }),
    ],
  });
  rpc.secrets.setSecret.mockResolvedValue({});
  const onAdded = vi.fn();
  render(<SecretsPage add="NPM_TOKEN" onAdded={onAdded} />);

  // Existing secrets show no value, their description under it, and who uses them.
  expect(await screen.findByText("Bot token")).toBeTruthy();
  // Blueprints the caller cannot see are only counted.
  expect(screen.getByText("build-box, 1 blueprint you cannot see")).toBeTruthy();
  expect(
    (screen.getByRole("button", { name: "Delete GITHUB_TOKEN" }) as HTMLButtonElement).disabled,
  ).toBe(true);

  const name = screen.getByLabelText("Name") as HTMLInputElement;
  const value = screen.getByLabelText("Value") as HTMLInputElement;
  expect(name.value).toBe("NPM_TOKEN");
  await waitFor(() => expect(document.activeElement).toBe(value));
  fireEvent.change(value, { target: { value: "npm_secret" } });
  fireEvent.change(screen.getByLabelText("Description"), { target: { value: "CI publish" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(rpc.secrets.setSecret).toHaveBeenCalledWith({
      name: "NPM_TOKEN",
      value: "npm_secret",
      description: "CI publish",
    }),
  );
  await waitFor(() => expect(onAdded).toHaveBeenCalled());
});
