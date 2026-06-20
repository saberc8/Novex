import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Novex Chat",
  description: "Novex customer-facing model and knowledge chat workspace"
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
