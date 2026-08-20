import { atom } from "nanostores";

export type User = { name: string; email: string } | null;

export const user = atom<User>(null);

export type LicenseStatus = {
  licensed: boolean;
  userId: string | null;
  email: string | null;
  reason: string | null;
} | null;

/** Result of `license_check_local` on boot. Null until the first check returns. */
export const licenseStatus = atom<LicenseStatus>(null);
