import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

/** @type {import("vite").UserConfig} */
export default defineConfig({
    root: path.join(process.cwd(), "src"),
    plugins: [wasm(), tailwindcss()],
    // https://github.com/rerun-io/rerun/issues/6815
    optimizeDeps: {
        exclude:
            process.env.NODE_ENV === "production"
                ? []
                : ["@rerun-io/web-viewer"],
    },
    base: "/bc",
    server: {
        port: 5173,
        host: "0.0.0.0",
        allowedHosts: ["local.alexdias.dev"],
    },
    build: { outDir: path.join(process.cwd(), "static") },
});

if ("REPOSITORY" in process.env) {
    config.base = `/${process.env.REPOSITORY}/`;
}
