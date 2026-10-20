import type { Metadata, Viewport } from "next";
import { IBM_Plex_Sans } from "next/font/google";

import { Providers } from "@/components/providers";
import { themeBootScript } from "@/components/theme-toggle";

import "./globals.css";

const plex = IBM_Plex_Sans({
  subsets: ["latin", "cyrillic"],
  weight: ["400", "500", "600"],
  variable: "--font-plex",
  display: "swap",
});

export const metadata: Metadata = {
  title: { default: "DealFlow", template: "%s - DealFlow" },
  description: "Sales pipeline, contacts and follow-ups for small B2B sales teams.",
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#f3f4f1" },
    { media: "(prefers-color-scheme: dark)", color: "#151816" },
  ],
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={plex.variable} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeBootScript }} />
      </head>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
