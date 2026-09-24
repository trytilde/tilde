import { ThemeToggle } from "./theme-toggle";
import { logout } from "@/auth";
import { TildeWordmark } from "@trytilde/connection-ui";
import { lazy, Suspense, useState } from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import {
  BotIcon,
  ChevronRightIcon,
  KeyRoundIcon,
  LogOutIcon,
  ShieldCheckIcon,
  UsersIcon,
} from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { useCaller } from "@/hooks/use-caller";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarFooter,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
} from "@/components/ui/sidebar";

const SidebarMaterial = lazy(() => import("./sidebar-material"));

export function AppSidebar() {
  const caller = useCaller();
  const [signingOut, setSigningOut] = useState(false);
  const [logoutError, setLogoutError] = useState("");
  async function signOut() {
    setSigningOut(true);
    setLogoutError("");
    try {
      await logout();
    } catch (error) {
      setLogoutError(error instanceof Error ? error.message : "Unable to sign out.");
    } finally {
      setSigningOut(false);
    }
  }
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  return (
    <Sidebar collapsible="offcanvas" variant="inset">
      <div className="tilde-sidebar-surface relative isolate flex h-full min-h-0 flex-col overflow-hidden rounded-lg">
        <Suspense fallback={null}>
          <SidebarMaterial />
        </Suspense>
        <div className="relative z-10 flex min-h-0 flex-1 flex-col">
          <SidebarHeader className="flex-row items-center justify-between">
            <Link to="/" aria-label="Tilde home" className="flex h-12 items-center gap-3 px-3">
              <TildeWordmark />
            </Link>
            <ThemeToggle />
          </SidebarHeader>
          <SidebarContent>
            <SidebarGroup>
              <SidebarGroupContent>
                <SidebarMenu className="gap-2">
                  <SidebarMenuItem>
                    <SidebarMenuButton
                      isActive={pathname === "/" || pathname.startsWith("/agent/")}
                      render={<Link to="/" />}
                    >
                      <BotIcon />
                      <span>Agent Registry</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                  {/* IAM groups the installation-wide identity pages. The group row only
                      opens and closes; it is never the active page itself. */}
                  <Collapsible
                    defaultOpen
                    className="group/collapsible"
                    render={<SidebarMenuItem />}
                  >
                    <CollapsibleTrigger render={<SidebarMenuButton />}>
                      <ShieldCheckIcon />
                      <span>IAM</span>
                      <ChevronRightIcon className="ml-auto transition-transform duration-200 group-data-open/collapsible:rotate-90" />
                    </CollapsibleTrigger>
                    <CollapsibleContent>
                      <SidebarMenuSub className="border-sidebar-foreground">
                        <SidebarMenuSubItem>
                          <SidebarMenuSubButton
                            isActive={pathname.startsWith("/api-keys")}
                            render={<Link to="/api-keys" />}
                          >
                            <KeyRoundIcon />
                            <span>API keys</span>
                          </SidebarMenuSubButton>
                        </SidebarMenuSubItem>
                        {caller.admin && (
                          <SidebarMenuSubItem>
                            <SidebarMenuSubButton
                              isActive={pathname.startsWith("/groups")}
                              render={<Link to="/groups" />}
                            >
                              <UsersIcon />
                              <span>Groups</span>
                            </SidebarMenuSubButton>
                          </SidebarMenuSubItem>
                        )}
                      </SidebarMenuSub>
                    </CollapsibleContent>
                  </Collapsible>
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
          <SidebarFooter className="mt-auto">
            {logoutError && (
              <p role="alert" className="px-2 text-xs text-destructive">
                {logoutError}
              </p>
            )}
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={() => void signOut()}
                  disabled={signingOut}
                  aria-busy={signingOut}
                >
                  <LogOutIcon />
                  <span>Sign out</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarFooter>
        </div>
      </div>
    </Sidebar>
  );
}
