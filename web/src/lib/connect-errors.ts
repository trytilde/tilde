import { ConnectError, type Interceptor } from "@connectrpc/connect";

/**
 * ConnectError's `message` is `[code] message` ("[permission_denied] You do not have permission
 * to do that."), and the pages show `error.message` as is. Drop the code prefix so people read
 * the server's sentence; `code` and `rawMessage` are unchanged for callers that branch on them.
 */
export const readableErrors: Interceptor = (next) => async (request) => {
  try {
    return await next(request);
  } catch (cause) {
    if (cause instanceof ConnectError && cause.rawMessage) cause.message = cause.rawMessage;
    throw cause;
  }
};
