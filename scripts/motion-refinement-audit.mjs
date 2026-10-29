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
  out = path.resolve(".jury/motion-refinement"),
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
      const page = await context.newPage();
      await page.goto(`http://127.0.0.1:${server.address().port}/`);
      await page
        .getByRole("button", { name: "List view", exact: true })
        .click();
      await page.waitForTimeout(250);
      const samples = await page.evaluate(async () => {
        const item = document
          .querySelector('button[aria-label="Open Photography Course"]')
          .closest("li");
        const sort = [
          ...document.querySelectorAll(".table-heading button"),
        ].find((e) => e.textContent.trim() === "Name");
        sort.click();
        const frames = [];
        for (let i = 0; i < 18; i++) {
          await new Promise(requestAnimationFrame);
          frames.push(item.getBoundingClientRect().y);
        }
        return frames;
      });
      const unique = new Set(samples.map(Math.round)).size;
      if (motion === "no-preference" && unique < 3)
        throw Error("Sort did not move smoothly");
      if (motion === "reduce" && unique > 2)
        throw Error("Reduced sort animated");
      await page.screenshot({
        path: path.join(out, `${theme}-${motion}-sorted.png`),
      });
      await page
        .getByRole("button", { name: "Card view", exact: true })
        .click();
      await page.waitForTimeout(250);
      if (
        await page
          .locator(".cask-table-scroll")
          .evaluate((e) => e.scrollWidth > e.clientWidth + 1)
      )
        throw Error("View overflow");
      reports.push({ theme, motion, unique });
      await context.close();
    }
  const context = await browser.newContext({
    viewport: { width: 560, height: 900 },
    hasTouch: true,
    isMobile: true,
  });
  await context.addInitScript(installFixture, { theme: "dark" });
  const page = await context.newPage();
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.getByRole("button", { name: "List view", exact: true }).click();
  await page.locator(".cask-row").first().hover();
  await page.waitForTimeout(220);
  if (
    (await page
      .locator(".cask-table li")
      .first()
      .evaluate((e) => getComputedStyle(e).boxShadow)) !== "none"
  )
    throw Error("Touch hover shadow retained");
  await context.close();
  await fs.writeFile(
    path.join(out, "result.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log("Motion refinement checks passed");
} finally {
  await browser.close();
  server.close();
}
