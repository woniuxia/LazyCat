export interface HotkeyAction {
  shortcut: string;
  action: string;
}

export interface ShortcutSuspect {
  appId: string;
  displayName: string;
  confidence: "high" | "low";
  matchedHotkeys: HotkeyAction[];
}

export interface SuspectApp {
  processName: string;
  displayName: string;
}

export interface HotkeyResult {
  shortcut: string;
  available: boolean;
  suspects: ShortcutSuspect[];
}

export interface CheckResponse {
  shortcut: string;
  available: boolean;
  suspects: ShortcutSuspect[];
  suspectedOwners: SuspectApp[];
}

export interface ScanResponse {
  results: HotkeyResult[];
  scannedCount: number;
  occupiedCount: number;
  suspectedOwners: SuspectApp[];
}

export interface ModifierGroup {
  key: string;
  label: string;
  items: HotkeyResult[];
}

export interface DetectOwnerResponse {
  shortcut: string;
  detected: boolean;
  owner?: {
    pid: number;
    processName: string;
    windowTitle: string;
    exePath: string;
  };
  signals: {
    foregroundChanged: boolean;
    newWindowAppeared: boolean;
    clipboardChanged: boolean;
  };
  confidence: "high" | "medium" | "low" | "none";
}
