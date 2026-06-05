import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Novex Agent",
  description: "Novex customer-facing agent workspace"
};

export default function RootLayout({
  children
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
