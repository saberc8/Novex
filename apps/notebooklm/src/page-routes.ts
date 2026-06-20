export type AppRouteKey = "knowledge" | "knowledge-sources" | "chat" | "chat-history" | "settings";

export interface AppRouteLink {
  key: AppRouteKey;
  label: string;
  href: string;
}

export const appRouteLinks: AppRouteLink[] = [];
