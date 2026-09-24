import { createRoot } from "react-dom/client";
import { BrokeringPage } from "@/routes/connections/-connection-setup-host";
const setupId = window.location.pathname.split("/").at(-1)!;
createRoot(document.getElementById("root")!).render(<BrokeringPage setupId={setupId} />);
