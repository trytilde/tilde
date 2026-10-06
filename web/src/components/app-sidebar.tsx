import { ThemeToggle } from "./theme-toggle";
import { TildeWordmark } from "@trytilde/connection-ui";
import { lazy, Suspense } from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import {
  BookOpenIcon,
  BotIcon,
  BoxIcon,
  ChevronRightIcon,
  PlugIcon,
  ServerIcon,
  WrenchIcon,
} from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
} from "@/components/ui/sidebar";

const SidebarMaterial = lazy(() => import("./sidebar-material"));

export function AppSidebar() {
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
                  <SidebarMenuItem>
                    <SidebarMenuButton
                      isActive={pathname.startsWith("/skills")}
                      render={<Link to="/skills" />}
                    >
                      <BookOpenIcon />
                      <span>Skills</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                  {/* Tools: the tool connections agents use, and the remote servers behind them. */}
                  <Collapsible
                    defaultOpen
                    className="group/collapsible"
                    render={<SidebarMenuItem />}
                  >
                    <CollapsibleTrigger render={<SidebarMenuButton />}>
                      <WrenchIcon />
                      <span>Tools</span>
                      <ChevronRightIcon className="ml-auto transition-transform duration-200 group-data-open/collapsible:rotate-90" />
                    </CollapsibleTrigger>
                    <CollapsibleContent>
                      <SidebarMenuSub className="border-sidebar-foreground">
                        <SidebarMenuSubItem>
                          <SidebarMenuSubButton
                            isActive={
                              pathname.startsWith("/tools") &&
                              !pathname.startsWith("/tools/remote-servers")
                            }
                            render={<Link to="/tools/connections" />}
                          >
                            <PlugIcon />
                            <span>Connections</span>
                          </SidebarMenuSubButton>
                        </SidebarMenuSubItem>
                        <SidebarMenuSubItem>
                          <SidebarMenuSubButton
                            isActive={pathname.startsWith("/tools/remote-servers")}
                            render={<Link to="/tools/remote-servers" />}
                          >
                            <ServerIcon />
                            <span>Remote servers</span>
                          </SidebarMenuSubButton>
                        </SidebarMenuSubItem>
                      </SidebarMenuSub>
                    </CollapsibleContent>
                  </Collapsible>
                  <SidebarMenuItem>
                    <SidebarMenuButton
                      isActive={pathname.startsWith("/sandboxes")}
                      render={<Link to="/sandboxes" />}
                    >
                      <BoxIcon />
                      <span>Sandboxes</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
        </div>
      </div>
    </Sidebar>
  );
}
