import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { ResourceKind } from "@trytilde/contracts/tilde/types/v1/authorization_pb.js";
import { iam } from "@/client";

/** What the signed-in user may do installation-wide. The server enforces; this only hides controls. */
export type Caller = {
  userId: string;
  admin: boolean;
  groupIds: string[];
  creatable: ResourceKind[];
};
const none: Caller = { userId: "", admin: false, groupIds: [], creatable: [] };
const CallerContext = createContext<Caller>(none);

export function CallerProvider({ children }: { children: ReactNode }) {
  const [caller, setCaller] = useState(none);
  useEffect(() => {
    const abort = new AbortController();
    void iam
      .getCaller({}, { signal: abort.signal })
      .then((response) =>
        setCaller({
          userId: response.userId ?? "",
          admin: response.admin,
          groupIds: response.groupIds,
          creatable: response.creatable,
        }),
      )
      .catch(() => undefined);
    return () => abort.abort();
  }, []);
  return <CallerContext.Provider value={caller}>{children}</CallerContext.Provider>;
}
export const useCaller = () => useContext(CallerContext);
