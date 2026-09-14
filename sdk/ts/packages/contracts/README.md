# @trytilde/contracts

Generated TypeScript Protobuf messages and ConnectRPC service descriptors for every
Tilde API surface. `pnpm build` runs `buf generate` against the repository's `proto/`
module into `gen/`; the output is not committed. Import files by their proto path:

```ts
import { AgentService } from "@trytilde/contracts/tilde/management/v1/agents_pb.js";
```

The web app and `@trytilde/sdk` both depend on this package instead of carrying copies.
