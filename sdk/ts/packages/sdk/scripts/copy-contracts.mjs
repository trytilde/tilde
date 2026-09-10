// Preserve protoc's declarations; tsc's JS inference cannot reconstruct protobuf message types.
import { cpSync, rmSync } from "node:fs";
rmSync("dist/gen", { recursive: true, force: true });
cpSync("src/gen", "dist/gen", { recursive: true });
