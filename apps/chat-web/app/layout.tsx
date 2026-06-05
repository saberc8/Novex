import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Novex Knowledge",
  description: "Novex customer-facing knowledge chat workspace"
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
