import Library from "./routes/Library.svelte";
import EditIntermediate from "./routes/EditIntermediate.svelte";
import HomeRedirect from "./routes/HomeRedirect.svelte";

export default {
  "/": HomeRedirect,
  "/library": Library,
  "/edit": EditIntermediate,
  "/clips": Library,
  "/grade": EditIntermediate,
  "*": HomeRedirect,
};
