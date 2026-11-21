import { createFileRoute, redirect, type AnyRedirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/")({
  beforeLoad: () => {
    let home: AnyRedirect = redirect({ to: "/registry", replace: true });
    throw home;
  },
});
