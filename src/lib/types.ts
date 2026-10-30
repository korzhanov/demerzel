// Типы, которые отдают команды Tauri (src-tauri/src/lib.rs).

export type ProcessStatus =
  | "Stopped"
  | "Starting"
  | "Running"
  | "Crashed"
  | { Exited: number };

export interface Project {
  name: string;
  path: string;
  children: Project[];
}

export interface ProcessRow {
  id: number;
  project: string;
  command: string;
  status: ProcessStatus;
}

export interface LogLine {
  stream: "stdout" | "stderr";
  text: string;
}

export function statusLabel(s: ProcessStatus): string {
  if (typeof s === "string") return s;
  return `Exited(${s.Exited})`;
}

export function isRunning(s: ProcessStatus): boolean {
  return s === "Running" || s === "Starting";
}
