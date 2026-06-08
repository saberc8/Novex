import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Developer Agent Workbench",
  description: "A Codex-like developer agent workbench web POC"
};

export default function RootLayout({
  children
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="zh-CN">
      <body>{children}</body>
    </html>
  );
}
