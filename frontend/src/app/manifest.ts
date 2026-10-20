import type { MetadataRoute } from "next";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "DealFlow CRM",
    short_name: "DealFlow",
    description: "Sales pipeline, contacts and follow-ups for small B2B sales teams.",
    start_url: "/today",
    display: "standalone",
    background_color: "#eef1f5",
    theme_color: "#1b2130",
    icons: [{ src: "/icon.svg", sizes: "any", type: "image/svg+xml", purpose: "any" }],
  };
}
