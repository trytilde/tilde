import { createPortal } from "react-dom";
import {
  createContext,
  useContext,
  useMemo,
  useState,
  type Dispatch,
  type ReactNode,
  type SetStateAction,
} from "react";

type AgentCrumb = { id: string; name: string };
const BreadcrumbContext = createContext<{
  agent?: AgentCrumb;
  navigationHost: HTMLDivElement | null;
  setNavigationHost: Dispatch<SetStateAction<HTMLDivElement | null>>;
  setAgent: Dispatch<SetStateAction<AgentCrumb | undefined>>;
} | null>(null);

/** Route data supplies the display name without making the dashboard fetch the agent again. */
export function DashboardBreadcrumbProvider({ children }: { children: ReactNode }) {
  const [agent, setAgent] = useState<AgentCrumb>();
  const [navigationHost, setNavigationHost] = useState<HTMLDivElement | null>(null);
  const value = useMemo(
    () => ({ agent, setAgent, navigationHost, setNavigationHost }),
    [agent, navigationHost],
  );
  return <BreadcrumbContext.Provider value={value}>{children}</BreadcrumbContext.Provider>;
}
export function useDashboardBreadcrumbs() {
  const value = useContext(BreadcrumbContext);
  if (!value) throw new Error("DashboardBreadcrumbProvider is missing");
  return value;
}

/** Pages contribute header navigation while retaining their React contexts and local state.
 * Standalone page renders keep the navigation inline when no dashboard host is present.
 */
export function DashboardNavigation({ children }: { children: ReactNode }) {
  const context = useContext(BreadcrumbContext);
  return context?.navigationHost ? createPortal(children, context.navigationHost) : <>{children}</>;
}
