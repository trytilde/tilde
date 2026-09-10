import { createHash, timingSafeEqual } from "node:crypto";
import { createServer } from "node:http";
import { Code, ConnectError } from "@connectrpc/connect";
import { connectNodeAdapter } from "@connectrpc/connect-node";
import type { MessageInitShape } from "@bufbuild/protobuf";
import {
  ConnectionProviderService,
  type HandleSetupRequest,
  HandleSetupResponseSchema,
} from "./gen/tilde/provider/v1/connections_pb.js";
export { ConnectionProviderService } from "./gen/tilde/provider/v1/connections_pb.js";
/** Optional hook for exceptional provisioning. Static keys and standard OAuth need no hook. */
export function createConnectionProviderServer(options: {
  backendToken: string;
  /** Persist actionId deduplication in your backend before performing non-idempotent provisioning. */
  handleSetup: (
    request: HandleSetupRequest,
    signal: AbortSignal,
  ) => Promise<MessageInitShape<typeof HandleSetupResponseSchema>>;
}) {
  if (options.backendToken.length < 32)
    throw new Error("A dedicated provider backend token is required");
  const expected = createHash("sha256").update(`Bearer ${options.backendToken}`).digest();
  return createServer(
    connectNodeAdapter({
      routes: (router) =>
        router.service(ConnectionProviderService, {
          async handleSetup(request, context) {
            const actual = createHash("sha256")
              .update(context.requestHeader.get("authorization") ?? "")
              .digest();
            if (!timingSafeEqual(expected, actual))
              throw new ConnectError("Provider authentication required", Code.Unauthenticated);
            return options.handleSetup(request, context.signal);
          },
        }),
    }),
  );
}
