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
  out = path.resolve(".jury/shared-motion"),
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
    for (const motion of ["reduce", "no-preference"]) {
      const context = await browser.newContext({
        viewport: { width: 1024, height: 900 },
        reducedMotion: motion,
      });
      await context.addInitScript(installFixture, { theme });
      const page = await context.newPage(),
        errors = [];
      page.on("pageerror", (e) => errors.push(e.message));
      await page.goto(`http://127.0.0.1:${server.address().port}/`);
      await page.getByRole("button", { name: "New Cask", exact: true }).click();
      const dialog = page.getByRole("dialog");
      await dialog.waitFor();
      await page.waitForTimeout(230);
      await page.screenshot({
        path: path.join(out, `${theme}-${motion}-dialog.png`),
      });
      await page.keyboard.press("Escape");
      await dialog.waitFor({ state: "detached" });
      if (
        !(await page
          .getByRole("button", { name: "New Cask", exact: true })
          .evaluate((e) => e === document.activeElement))
      )
        throw Error("Dialog focus not restored");
      await page.goto(
        `http://127.0.0.1:${server.address().port}/?cask=qa-cask-0`,
      );
      await page
        .getByRole("button", { name: "Cask actions", exact: true })
        .click();
      await page.getByRole("menu").waitFor();
      await page.waitForTimeout(220);
      await page.keyboard.press("Escape");
      await page.getByRole("menu").waitFor({ state: "detached" });
      if (
        !(await page
          .getByRole("button", { name: "Cask actions", exact: true })
          .evaluate((e) => e === document.activeElement))
      )
        throw Error("Menu focus");
      await page.getByRole("button", { name: "Settings", exact: true }).click();
      const summary = page.locator('summary[aria-label="Cask settings"]');
      await summary.click();
      await page.waitForTimeout(220);
      await page
        .getByRole("textbox", { name: "Cask name", exact: true })
        .waitFor();
      await summary.click();
      await page.waitForTimeout(220);
      const height = await page
        .locator(".settings-disclosure")
        .first()
        .evaluate((e) => getComputedStyle(e, "::details-content").blockSize);
      if (height !== "0px")
        throw Error("Disclosure did not collapse " + height);
      await summary.click();
      await summary.click();
      await summary.click();
      await page.waitForTimeout(220);
      if (
        !(await page
          .locator(".settings-disclosure")
          .first()
          .evaluate((e) => e.open))
      )
        throw Error("Rapid disclosure");
      await page.screenshot({
        path: path.join(out, `${theme}-${motion}-expanded.png`),
      });
      await page.emulateMedia({ reducedMotion: "reduce" });
      const durations = await page
        .getByRole("button", { name: "Change appearance", exact: true })
        .evaluate((e) => getComputedStyle(e).transitionDuration);
      if (durations.split(",").some((v) => parseFloat(v) > 0))
        throw Error("Live reduced motion");
      await page
        .getByRole("button", { name: "Cask actions", exact: true })
        .click();
      await page.waitForTimeout(30);
      if (
        await page
          .getByRole("menu")
          .evaluate((e) =>
            e.getAnimations().some((a) => a.playState === "running"),
          )
      )
        throw Error("Menu ignores live motion");
      await page.keyboard.press("Escape");
      if (errors.length) throw Error(errors.join());
      reports.push({ theme, motion, height, durations });
      await context.close();
    }
  await fs.writeFile(
    path.join(out, "result.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log("Shared motion interaction audit passed");
} finally {
  await browser.close();
  server.close();
}
