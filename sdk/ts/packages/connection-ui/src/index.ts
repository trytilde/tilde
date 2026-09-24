// Small, optional building blocks. Providers remain ordinary React apps, not form descriptors.
export { useSetup, useConnectionForm, Form, Page, mount, type Setup } from "./setup.js";
export { Button, buttonVariants } from "./components/button.js";
export { Input } from "./components/input.js";
export { Label, Textarea, NativeSelect } from "./components/controls.js";
export { useForm, Controller, FormProvider, useFormContext } from "react-hook-form";
export { AuthDriver, type Brokering, type SetupValues } from "@trytilde/sdk/connection-setup";

export { StandardSetup, CredentialForm } from "./standard.js";

export { TildeWordmark } from "./wordmark.js";

export { LoadingReveal } from "./loading-reveal.js";

export { ProviderPage } from "./provider-page.js";
