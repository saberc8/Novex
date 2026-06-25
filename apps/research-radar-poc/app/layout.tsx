import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Research Radar POC",
  description: "AI research direction tracking POC powered by Novex Agent"
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
