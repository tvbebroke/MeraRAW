declare module "liquid-glass-svelte/GlassedButton.svelte" {
  import { SvelteComponent } from "svelte";
  const component: any;
  export default component;
}

declare module "posthog-js/dist/module.no-external" {
  import type { PostHog } from "posthog-js";
  export type { PostHog };
  const posthog: PostHog;
  export default posthog;
}

