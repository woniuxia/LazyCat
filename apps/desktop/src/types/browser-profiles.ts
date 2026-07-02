export type BrowserProfileBrowser = "edge";

export interface BrowserProfileItem {
  browser: BrowserProfileBrowser;
  profileDir: string;
  edgeDisplayName: string;
  alias: string;
  hidden: boolean;
  launchCount: number;
  lastLaunchedAt: string | null;
}

export interface BrowserProfilesListResponse {
  edgeFound: boolean;
  edgePath: string | null;
  userDataDir: string;
  probedEdgePaths: string[];
  warnings: string[];
  profiles: BrowserProfileItem[];
}

export interface BrowserProfilesLaunchResponse {
  ok: true;
  launchCount: number;
  lastLaunchedAt: string;
  warnings: string[];
}
