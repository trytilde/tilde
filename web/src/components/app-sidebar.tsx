import { logout } from "@/auth";
import { TildeWordmark } from "@trytilde/connection-ui";
import { lazy, Suspense, useState } from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import { BotIcon, LogOutIcon } from "lucide-react";
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
} from "@/components/ui/sidebar";

const SidebarMaterial = lazy(() => import("./sidebar-material"));

export function AppSidebar() {
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
          <SidebarHeader>
            <Link to="/" aria-label="Tilde home" className="flex h-12 items-center gap-3 px-3">
              <TildeWordmark />
            </Link>
          </SidebarHeader>
          <SidebarContent>
            <SidebarGroup>
              <SidebarGroupContent>
                <SidebarMenu>
                  <SidebarMenuItem>
                    <SidebarMenuButton
                      isActive={pathname === "/" || pathname.startsWith("/agent/")}
                      render={<Link to="/" />}
                    >
                      <BotIcon />
                      <span>Agent Registry</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
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
