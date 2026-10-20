import type { MetadataRoute } from "next";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "DealFlow CRM",
    short_name: "DealFlow",
    description: "Sales pipeline, contacts and follow-ups for small B2B sales teams.",
    start_url: "/today",
    display: "standalone",
    background_color: "#f4f5f7",
    theme_color: "#2440f0",
    icons: [{ src: "/icon.svg", sizes: "any", type: "image/svg+xml", purpose: "any" }],
  };
}
