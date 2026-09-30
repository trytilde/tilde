/**
 * Example remote tool server (TypeScript): a small in-memory CRM. It needs credentials, so it
 * is used through instances: each instance is one workspace, unlocked by an API key such as
 * `demo-acme`. Tilde runs the setup, asks `verify` to accept the key, and sends the instance's
 * key with every call as `ctx.auth`.
 */
import * as z from "zod";
import { createToolHost, defineAuth } from "@trytilde/sdk/tool-host";

type Customer = { id: string; name: string; email: string; notes: string[] };
const workspaces = new Map<string, Customer[]>();
function workspace(key: string) {
  let customers = workspaces.get(key);
  if (!customers) {
    customers = [
      { id: "c1", name: "Ada Lovelace", email: "ada@example.com", notes: [] },
      { id: "c2", name: "Grace Hopper", email: "grace@example.com", notes: ["Prefers email"] },
    ];
    workspaces.set(key, customers);
  }
  return customers;
}

const auth = defineAuth({
  provider: {
    id: "example-crm",
    name: "Example CRM",
    instructions:
      "Any key that starts with demo- works, for example demo-acme. Each key is its own workspace.",
    accountNameLabel: "Workspace name",
  },
  methods: {
    api_key: {
      name: "API key",
      schema: z.object({ api_key: z.string().min(1).meta({ title: "API key", writeOnly: true }) }),
    },
  },
  async verify({ api_key }) {
    if (!api_key.startsWith("demo-")) throw new Error("Example CRM keys start with demo-");
    return { accountLabel: api_key.slice("demo-".length) };
  },
});

const customer = z.object({
  id: z.string(),
  name: z.string(),
  email: z.string(),
  notes: z.array(z.string()),
});

createToolHost({
  auth,
  tools: {
    search_customers: auth.tool({
      description: "Find customers whose name or email contains the query.",
      summary: "Searched customers",
      inputSchema: z.object({ query: z.string().describe("Part of a name or email address") }),
      outputSchema: z.object({ customers: z.array(customer) }),
      annotations: { readOnly: true },
      run({ query }, ctx) {
        const needle = query.toLowerCase();
        return {
          customers: workspace(ctx.auth.api_key).filter((c) =>
            `${c.name} ${c.email}`.toLowerCase().includes(needle),
          ),
        };
      },
    }),
    add_customer: auth.tool({
      description: "Add a customer to the workspace.",
      summary: "Added a customer",
      inputSchema: z.object({ name: z.string().min(1), email: z.email() }),
      outputSchema: customer,
      run({ name, email }, ctx) {
        const customers = workspace(ctx.auth.api_key);
        const added = { id: `c${customers.length + 1}`, name, email, notes: [] };
        customers.push(added);
        return added;
      },
    }),
    add_note: auth.tool({
      description: "Attach a note to a customer.",
      summary: "Added a note",
      inputSchema: z.object({ customer_id: z.string(), note: z.string().min(1) }),
      outputSchema: customer,
      run({ customer_id, note }, ctx) {
        const found = workspace(ctx.auth.api_key).find((c) => c.id === customer_id);
        if (!found) throw new Error(`No customer ${customer_id}`);
        found.notes.push(note);
        return found;
      },
    }),
  },
});
console.log("Example CRM tool server is dialing in to Tilde");
