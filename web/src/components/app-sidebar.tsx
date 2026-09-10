import { BotIcon, PlugIcon, MessageCircleIcon } from "lucide-react";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";

export function AppSidebar() {
  return (
    <Sidebar collapsible="offcanvas" variant="inset">
      <SidebarHeader>
        <a href="/" aria-label="Tilde home" className="flex h-12 items-center gap-3 px-3">
          <img src="/tilde-mark.svg" alt="" className="size-7 shrink-0" />
          <span className="font-mono text-xl tracking-wide">tilde</span>
        </a>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  isActive={window.location.pathname === "/"}
                  render={<a href="/" aria-current="page" />}
                >
                  <BotIcon />
                  <span>Agent Registry</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  isActive={window.location.pathname.startsWith("/connections")}
                  render={<a href="/connections" />}
                >
                  <PlugIcon />
                  <span>Connections</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  isActive={window.location.pathname === "/chat"}
                  render={<a href="/chat" />}
                >
                  <MessageCircleIcon />
                  <span>Chat</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
}
