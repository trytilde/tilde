import { lazy, Suspense } from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import {
  BookOpenIcon,
  BotIcon,
  BoxIcon,
  BrainIcon,
  NotebookTextIcon,
  PlugIcon,
  ServerIcon,
} from "lucide-react";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from "@/components/ui/sidebar";

const SidebarMaterial = lazy(() => import("./sidebar-material"));

const registry = [
  { to: "/registry", label: "Agent Registry", icon: BotIcon },
  { to: "/registry/skills", label: "Skills", icon: BookOpenIcon },
  { to: "/registry/tools/connections", label: "Connections", icon: PlugIcon },
  { to: "/registry/tools/remote-servers", label: "Remote servers", icon: ServerIcon },
  { to: "/registry/sandboxes", label: "Sandboxes", icon: BoxIcon },
  { to: "/registry/memory", label: "Memory", icon: BrainIcon },
  { to: "/registry/wikis", label: "Wikis", icon: NotebookTextIcon },
] as const;

function active(pathname: string, to: (typeof registry)[number]["to"]) {
  if (to === "/registry") return pathname === to || pathname.startsWith("/registry/agent/");
  // Tool connections own every tools page except remote servers, and Connections → Secrets.
  if (to === "/registry/tools/connections")
    return (
      pathname.startsWith("/connections/secrets") ||
      (pathname.startsWith("/registry/tools") &&
        !pathname.startsWith("/registry/tools/remote-servers"))
    );
  return pathname.startsWith(to);
}

/** The app's sidebar; it collapses to icons, with each item's name as its tooltip. */
export function AppSidebar() {
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  let platform = true;
  return (
    <Sidebar collapsible="icon" variant="inset">
      <div className="tilde-sidebar-surface relative isolate flex h-full min-h-0 flex-col overflow-hidden rounded-lg">
        <Suspense fallback={null}>
          <SidebarMaterial />
        </Suspense>
        <div className="relative z-10 flex min-h-0 flex-1 flex-col">
          <SidebarHeader></SidebarHeader>
          <SidebarContent>
            {platform && (
              <SidebarGroup>
                <SidebarGroupContent>
                  <SidebarMenu>
                    {registry.map(({ to, label, icon: Icon }) => (
                      <SidebarMenuItem key={to}>
                        <SidebarMenuButton
                          isActive={active(pathname, to)}
                          tooltip={label}
                          render={<Link to={to} />}
                        >
                          <Icon />
                          <span>{label}</span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    ))}
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            )}
          </SidebarContent>
        </div>
      </div>
      <SidebarRail />
    </Sidebar>
  );
}
