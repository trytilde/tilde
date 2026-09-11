import { DashboardBreadcrumbProvider } from "@/components/dashboard-breadcrumbs";
import { createFileRoute, Outlet } from "@tanstack/react-router";
import { Auth } from "@/components/auth";
import { AppSidebar } from "@/components/app-sidebar";
import { SiteHeader } from "@/components/site-header";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";

export const Route = createFileRoute("/_app")({ component: AppLayout });

function AppLayout() {
  return (
    <Auth>
      <DashboardBreadcrumbProvider>
        <TooltipProvider>
          <SidebarProvider>
            <AppSidebar />
            <SidebarInset>
              <SiteHeader />
              <main className="flex flex-1 flex-col">
                <Outlet />
              </main>
            </SidebarInset>
          </SidebarProvider>
        </TooltipProvider>
      </DashboardBreadcrumbProvider>
    </Auth>
  );
}
