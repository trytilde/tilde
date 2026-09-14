import type { Brokering } from "./gen/tilde/setup/v1/connections_pb.js";
export type SetupValues = Record<string, string>;
type Reply = { id: number; state?: Brokering; error?: string };
/** Browser-only client for an isolated provider iframe. The host owns all credentials and scope.
 * @example
 * const client = createConnectionSetupClient();
 * let setup = await client.getSetup();
 * setup = await client.saveDraft(setup.actionId, { workspace: "personal" });
 * await client.saveCredentials(setup.actionId, { api_key: userSuppliedKey });
 * client.dispose();
 */
export function createConnectionSetupClient() {
  let port: MessagePort | undefined;
  let nextId = 0;
  let closed = false;
  const pending = new Map<
    number,
    { resolve: (state: Brokering) => void; reject: (error: Error) => void }
  >();
  let readyResolve!: () => void;
  let readyReject!: (error: Error) => void;
  const ready = new Promise<void>((resolve, reject) => {
    readyResolve = resolve;
    readyReject = reject;
  });
  const timeout = setTimeout(
    () => readyReject(new Error("Connection setup host did not respond")),
    10000,
  );
  const receive = (event: MessageEvent) => {
    if (
      event.source !== window.parent ||
      event.data?.type !== "tilde.setup.port" ||
      port ||
      !event.ports[0]
    )
      return;
    port = event.ports[0];
    port.onmessage = ({ data }: MessageEvent<Reply>) => {
      const request = pending.get(data.id);
      if (!request) return;
      pending.delete(data.id);
      if (data.error) request.reject(new Error(data.error));
      else if (data.state) request.resolve(data.state);
      else request.reject(new Error("Invalid setup response"));
    };
    clearTimeout(timeout);
    readyResolve();
  };
  window.addEventListener("message", receive);
  window.parent.postMessage({ type: "tilde.setup.ready" }, "*");
  async function call(
    method: string,
    actionId?: string,
    fields?: SetupValues,
    action?: string,
    connectionName?: string,
  ): Promise<Brokering> {
    await ready;
    if (closed) throw new Error("Setup client closed");
    return new Promise((resolve, reject) => {
      const id = ++nextId;
      pending.set(id, { resolve, reject });
      port!.postMessage({ id, method, actionId, fields, action, connectionName });
    });
  }
  return {
    getSetup: () => call("read"),
    setConnectionName: (actionId: string, name: string) =>
      call("setConnectionName", actionId, undefined, undefined, name),
    saveDraft: (actionId: string, draft: SetupValues) => call("saveDraft", actionId, draft),
    startOAuth: (actionId: string, fields: SetupValues) => call("startOAuth", actionId, fields),
    saveCredentials: (actionId: string, fields: SetupValues) =>
      call("saveCredentials", actionId, fields),
    executeProviderAction: (actionId: string, action: string, fields: SetupValues = {}) =>
      call("executeProviderAction", actionId, fields, action),
    cancelSetup: () => call("cancel"),
    dispose: () => {
      closed = true;
      clearTimeout(timeout);
      window.removeEventListener("message", receive);
      port?.close();
      const error = new Error("Setup client closed");
      readyReject(error);
      for (const request of pending.values()) request.reject(error);
      pending.clear();
    },
  };
}

export { AuthDriver, type Brokering } from "./gen/tilde/setup/v1/connections_pb.js";
