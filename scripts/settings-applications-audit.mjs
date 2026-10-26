import fs from "node:fs/promises";
import path from "node:path";
import http from "node:http";
import { createRequire } from "node:module";
import { installFixture } from "./ui-fixture.mjs";
const require = createRequire(
    path.resolve("target/ui-audit-tools/package.json"),
  ),
  { chromium } = require("playwright"),
  axe = await fs.readFile(require.resolve("axe-core/axe.min.js"), "utf8"),
  out = path.resolve(".jury/settings-applications"),
  dist = path.resolve("apps/desktop/dist");
await fs.mkdir(out, { recursive: true });
const server = http.createServer(async (req, res) => {
  try {
    const pathname = new URL(req.url, "http://localhost").pathname;
    const file = path.join(dist, pathname === "/" ? "index.html" : pathname);
    res.setHeader(
      "content-type",
      {
        ".html": "text/html",
        ".js": "application/javascript",
        ".css": "text/css",
        ".png": "image/png",
        ".woff2": "font/woff2",
      }[path.extname(file)] || "application/octet-stream",
    );
    res.end(await fs.readFile(file));
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const browser = await chromium.launch({ channel: "chrome", headless: true }),
  reports = [];
try {
  for (const theme of ["light", "dark"])
    for (const width of [390, 560, 1024]) {
      const context = await browser.newContext({
        viewport: { width, height: 900 },
        reducedMotion: "reduce",
      });
      await context.addInitScript(installFixture, { theme, detail: true });
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", (e) => errors.push(e.message));
      await page.goto(
        `http://127.0.0.1:${server.address().port}/?cask=qa-cask-0`,
      );
      for (const tab of ["Applications", "Settings"]) {
        await page.getByRole("button", { name: tab, exact: true }).click();
        await page.screenshot({
          path: path.join(out, `${theme}-${width}-${tab}.png`),
        });
        await page.addScriptTag({ content: axe });
        const violations = (
          await page.evaluate(() =>
            window.axe.run(document, {
              runOnly: {
                type: "tag",
                values: ["wcag2a", "wcag2aa", "wcag21aa"],
              },
            }),
          )
        ).violations;
        if (violations.length)
          throw Error(JSON.stringify(violations.map((v) => v.id)));
        if (
          await page
            .locator(".workspace-content")
            .evaluate((e) => e.scrollWidth > e.clientWidth + 1)
        )
          throw Error("Overflow");
        if (tab === "Settings") {
          for (const name of ["Cask settings", "Categories", "Export Cask"]) {
            const summary = page.getByRole("button", { name, exact: true });
            await page.locator(`summary[aria-label="${name}"]`).click();
          }
          await page
            .getByRole("textbox", { name: "Cask name", exact: true })
            .fill("Changed");
          await page
            .getByRole("button", { name: "Reset changes", exact: true })
            .click();
          await page
            .getByRole("button", { name: "Change appearance", exact: true })
            .click();
          await page.locator(".appearance-settings-editor").waitFor();
        } else {
          await page
            .getByRole("button", { name: "References", exact: true })
            .click();
          await page.getByRole("button", { name: "All", exact: true }).click();
          await page
            .getByRole("textbox", { name: "Search Cask applications" })
            .fill("missing-xyz");
          await page
            .getByRole("heading", { name: "No matching applications" })
            .waitFor();
        }
      }
      if (errors.length) throw Error(errors.join());
      reports.push({ theme, width, passed: true });
      await context.close();
    }
  await fs.writeFile(
    path.join(out, "result.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log("Settings and applications checks passed");
} finally {
  await browser.close();
  server.close();
}
