declare module "posthog-js/dist/module.no-external" {
  import type { PostHog } from "posthog-js";
  export type { PostHog };
  const posthog: PostHog;
  export default posthog;
}
