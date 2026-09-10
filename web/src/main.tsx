import { ConversationsPage } from "@/components/conversations";
import { Auth } from "@/components/auth";
import React from "react";
import { ConnectionsPage, BrokeringPage } from "@/components/connections";
import { createRoot } from "react-dom/client";
import { AppSidebar } from "@/components/app-sidebar";
import { AgentRegistry } from "@/components/agent-registry";
import { SiteHeader } from "@/components/site-header";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import "./style.css";

const brokerMatch = window.location.pathname.match(/^\/connections\/broker\/([0-9a-f-]+)$/);
const chatPage = window.location.pathname === "/chat";
const connectionsPage = window.location.pathname === "/connections";

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {brokerMatch ? (
      <BrokeringPage setupId={brokerMatch[1]} />
    ) : (
      <Auth>
        <TooltipProvider>
          <SidebarProvider>
            <AppSidebar />
            <SidebarInset>
              <SiteHeader
                title={chatPage ? "Chat" : connectionsPage ? "Connections" : "Agent Registry"}
              />
              <main className="flex flex-1 flex-col">
                {chatPage ? (
                  <ConversationsPage />
                ) : connectionsPage ? (
                  <ConnectionsPage />
                ) : (
                  <AgentRegistry />
                )}
              </main>
            </SidebarInset>
          </SidebarProvider>
        </TooltipProvider>
      </Auth>
    )}
  </React.StrictMode>,
);
