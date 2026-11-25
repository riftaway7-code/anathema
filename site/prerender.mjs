// after `vite build` and the ssr build of src/entry-server.jsx: render the page to html and
// put it into dist/index.html, so the content and the hero image are in the first response.
import fs from "node:fs";
import { render } from "./dist-ssr/entry-server.js";

const file = "dist/index.html";
const html = fs.readFileSync(file, "utf8");
if (!html.includes("<!--app-->")) throw new Error("dist/index.html has no <!--app--> placeholder");
fs.writeFileSync(file, html.replace("<!--app-->", render()));
fs.rmSync("dist-ssr", { recursive: true, force: true });
console.log("prerendered dist/index.html");
