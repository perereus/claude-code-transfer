export interface SessionInfo {
  id: string;
  title: string;
  size: number;
  modifiedMs: number;
}

export interface ProjectInfo {
  slug: string;
  realPath: string;
  name: string;
  folderExists: boolean;
  folderSize: number;
  chatSize: number;
  lastModifiedMs: number;
  sessions: SessionInfo[];
}

export interface PreviewSession {
  id: string;
  title: string;
  existsLocally: boolean;
}

export interface ConfigFile {
  relPath: string;
  size: number;
}

export interface ConfigPreview {
  relPath: string;
  size: number;
  existsLocally: boolean;
}

export interface ConfigResolution {
  relPaths: string[];
  backupExisting: boolean;
}

export interface PreviewProject {
  slug: string;
  sourcePath: string;
  suggestedTargetPath: string;
  sessions: PreviewSession[];
  includesFiles: boolean;
  fileCount: number;
  filesSize: number;
  targetFolderExists: boolean;
  existingSessionCount: number;
}

export interface ArchivePreview {
  sourceHome: string;
  exportedAt: string;
  projects: PreviewProject[];
  config: ConfigPreview[];
}

export interface ExportSelection {
  slug: string;
  realPath: string;
  sessionIds: string[];
  includeFiles: boolean;
}

export interface ImportResolution {
  slug: string;
  importChats: boolean;
  sessionMode: "merge" | "overwrite";
  importFiles: boolean;
  onlyNewer: boolean;
  targetPath: string;
}

export interface ImportSummary {
  sessionsImported: number;
  sessionsSkipped: number;
  filesWritten: number;
  filesSkipped: number;
  historyLinesAdded: number;
  configWritten: number;
  projects: string[];
}

// human-readable labels for the whitelisted config files
export const CONFIG_LABELS: Record<string, string> = {
  "settings.json": "Settings (settings.json)",
  "settings.local.json": "Local settings (settings.local.json)",
  "keybindings.json": "Keybindings",
  "CLAUDE.md": "Global memory (CLAUDE.md)",
  "plugins/installed_plugins.json": "Installed plugins",
  "plugins/known_marketplaces.json": "Plugin marketplaces",
};

export interface Progress {
  message: string;
  current: number;
  total: number;
}

export const DEFAULT_EXCLUSIONS = [
  "node_modules",
  ".git",
  "venv",
  ".venv",
  "target",
  "dist",
  "__pycache__",
];

export function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}
