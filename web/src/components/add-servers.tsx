import { useState } from "react";
import { ChevronDownIcon, ServerIcon, ZapIcon } from "lucide-react";
import { ProviderSource } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { connections, toolHosts } from "@/client";
import { AddMcpServer, AUTH, type Auth } from "./add-mcp-server-dialog";
import { ChooseDialog } from "./choose-dialog";
import { pillClass } from "./skill-common";
import { Button } from "@/components/ui/button";
import {
  hostEntry,
  providerEntry,
  ToolProviderDialogs,
  type CatalogProvider,
} from "./tool-catalog";
import {
  DEPLOYMENTS,
  loadToolProviders,
  RegisterToolHostDialog,
  type Deployment,
} from "./tool-connections";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

/**
 * The "Add MCP server" and "Add Tilde tool server" actions. Each opens a menu: with `existing`,
 * "Choose existing" sets up a new account on a registered server; "Create new" lists the auth
 * (MCP) or deployment (tool server) methods, and the add dialog opens with the chosen one.
 */
export function AddServers({ existing, onChanged }: { existing: boolean; onChanged: () => void }) {
  const [auth, setAuth] = useState<Auth>();
  const [deployment, setDeployment] = useState<Deployment>();
  const [choosing, setChoosing] = useState<"mcp" | "host">();
  const [setup, setSetup] = useState<CatalogProvider>();
  let chooseExisting = existing;
  let authOptions: Record<string, string> = AUTH;
  let deploymentOptions: Record<string, string> = DEPLOYMENTS;
  return (
    <>
        <ServerMenu
          icon={<ServerIcon />}
          label="Add MCP server"
          createLabel="Create new · choose auth method"
          options={authOptions}
          onExisting={chooseExisting ? () => setChoosing("mcp") : undefined}
          onCreate={(key) => setAuth(key as Auth)}
        />
        <ServerMenu
          icon={<ZapIcon />}
          label="Add Tilde tool server"
          createLabel="Create new · choose deployment method"
          options={deploymentOptions}
          onExisting={chooseExisting ? () => setChoosing("host") : undefined}
          onCreate={(key) => setDeployment(key as Deployment)}
        />
      <AddMcpServer
        open={!!auth}
        auth={auth ?? "oauth"}
        onOpenChange={(open) => !open && setAuth(undefined)}
        onChanged={onChanged}
      />
      <RegisterToolHostDialog
        key={deployment}
        open={!!deployment}
        deployment={deployment ?? "connected"}
        onOpenChange={(open) => !open && setDeployment(undefined)}
        onRegistered={onChanged}
      />
      <ChooseDialog<CatalogProvider>
        open={!!choosing}
        title={choosing === "mcp" ? "Choose an MCP server" : "Choose a Tilde tool server"}
        description="Add a new account on a server already registered here."
        searchLabel="Search servers"
        listLabel="Registered servers"
        empty={(search) =>
          search ? "No servers match your search." : "No servers are registered yet."
        }
        busy={false}
        reloadKey={choosing}
        load={async (search, signal) => {
          if (choosing === "mcp") {
            const catalog = await loadToolProviders(
              { source: ProviderSource.MCP_SERVER, search },
              signal,
            );
            return catalog.providers.map((provider) => ({
              id: provider.id,
              name: provider.name,
              detail: provider.connectionTypes.map((type) => type.name).join(" · "),
              iconUrl: provider.iconUrl,
              value: providerEntry(provider, []),
            }));
          }
          // Only hosts that publish a provider have accounts to set up.
          const hosts = await toolHosts.listToolHosts({ search, withProvider: true }, { signal });
          const published = await Promise.all(
            hosts.toolHosts.map((host) =>
              connections.getProvider({ id: host.providerId ?? "" }, { signal }),
            ),
          );
          const providers = published.flatMap(({ provider }) => (provider ? [provider] : []));
          return hosts.toolHosts.map((host) => ({
            id: host.id,
            name: host.name,
            detail: host.authMethods.join(" · ") || "Tool server",
            value: hostEntry(host, providers, []),
          }));
        }}
        onClose={() => setChoosing(undefined)}
        onChoose={async (choice) => {
          setChoosing(undefined);
          setSetup(choice.value);
        }}
      />
      {setup && (
        <ToolProviderDialogs
          key={setup.id}
          entry={setup}
          start="setup"
          refresh={async () => onChanged()}
          onClose={() => setSetup(undefined)}
        />
      )}
    </>
  );
}

function ServerMenu({
  icon,
  label,
  createLabel,
  options,
  onExisting,
  onCreate,
}: {
  icon: React.ReactNode;
  label: string;
  createLabel: string;
  options: Record<string, string>;
  onExisting?: () => void;
  onCreate: (key: string) => void;
}) {
  const creatable = Object.keys(options).length > 0;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="outline" className={pillClass} />}>
        {icon}
        {label}
        <ChevronDownIcon />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-64">
        {onExisting && <DropdownMenuItem onClick={onExisting}>Choose existing</DropdownMenuItem>}
        {onExisting && creatable && <DropdownMenuSeparator />}
        {creatable && (
          <DropdownMenuGroup>
            <DropdownMenuLabel>
              {onExisting ? createLabel : createLabel.split(" · ")[1]}
            </DropdownMenuLabel>
            {Object.entries(options).map(([key, name]) => (
              <DropdownMenuItem key={key} onClick={() => onCreate(key)}>
                {name}
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
