import { atom } from "nanostores";

export type User = { name: string; email: string } | null;

// ponytail: placeholder until login screen + real auth exist
export const user = atom<User>(null);
