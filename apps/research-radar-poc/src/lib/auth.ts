const TOKEN_KEY = "novex_token";

export function getAuthToken() {
  if (typeof window === "undefined" || !window.localStorage) {
    return null;
  }
  return window.localStorage.getItem(TOKEN_KEY);
}
