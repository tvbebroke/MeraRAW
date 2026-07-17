import Library from "./routes/Library.svelte";
import EditIntermediate from "./routes/EditIntermediate.svelte";

export default {
  "/": EditIntermediate,
  "/library": Library,
  "/edit": EditIntermediate,
  "*": EditIntermediate,
};
