// Thin shim so shell modals can import pickFolder without reaching into ipc/.
export { pickFolder, pickFile } from "../ipc/commands";
