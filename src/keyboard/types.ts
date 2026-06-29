export type KeymapRelevance = "core" | "optional" | "out_of_scope";

export type KeymapScope =
  | "global"
  | "module:library"
  | "module:develop"
  | "secondary"
  | "module:book"
  | "module:slideshow"
  | "module:print"
  | "module:map"
  | "module:web"
  | `tool:${string}`;

export interface RawBinding {
  id: string;
  section: string;
  action: string;
  keys: { windows: string; macos: string };
  scope: string;
  relevance: KeymapRelevance;
  description: string;
  note: string;
}

export interface KeymapFile {
  source: string;
  note: string;
  scopes: string[];
  bindings: RawBinding[];
}

export interface ResolvedBinding {
  id: string;
  section: string;
  action: string;
  chord: string;
  scope: KeymapScope;
  relevance: KeymapRelevance;
  description: string;
  note: string;
  /** click-modifier, drag, etc. — not handled by key dispatcher */
  inputKind: "key" | "pointer";
}

export type CommandHandler = (binding: ResolvedBinding) => void | Promise<void>;

export interface DispatchResult {
  handled: boolean;
  binding?: ResolvedBinding;
  scope?: KeymapScope;
  reason?: "no-match" | "no-handler" | "blocked-input";
}
