import { defineConfig } from "vite";

// Served from https://johanneswilm.github.io/lingotweaker/ on GitHub Pages;
// override with BASE_URL for a local or differently rooted deployment.
export default defineConfig({
  base: process.env.BASE_URL ?? "/lingotweaker/",
  build: {
    target: "es2022",
    sourcemap: true,
  },
  worker: {
    format: "es",
  },
});
