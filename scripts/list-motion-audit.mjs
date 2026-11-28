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
  out = path.resolve(".jury/list-motion"),
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
    for (const width of [560, 1024, 1400])
      for (const motion of ["reduce", "no-preference"]) {
        const context = await browser.newContext({
          viewport: { width, height: 900 },
          reducedMotion: motion,
        });
        await context.addInitScript(installFixture, { theme, reference: true });
        const page = await context.newPage();
        await page.goto(`http://127.0.0.1:${server.address().port}/`);
        await page
          .getByRole("button", { name: "List view", exact: true })
          .click();
        const row = page.locator(".cask-row").first();
        await row.waitFor();
        await page.waitForTimeout(250);
        await page.screenshot({
          path: path.join(out, `${theme}-${width}-${motion}.png`),
        });
        const before = await row.boundingBox();
        const item = row.locator("..");
        await item.evaluate((e) => e.classList.add("selected"));
        await page.mouse.move(0, 0);
        await page.waitForTimeout(250);
        const rest = await item.evaluate((e) =>
          JSON.stringify({
            background: getComputedStyle(e).backgroundColor,
            border: getComputedStyle(e).borderTopColor,
            shadow: getComputedStyle(e).boxShadow,
          }),
        );
        await row.hover();
        await page.waitForTimeout(250);
        const after = await row.boundingBox();
        const hover = await item.evaluate((e) =>
          JSON.stringify({
            background: getComputedStyle(e).backgroundColor,
            border: getComputedStyle(e).borderTopColor,
            shadow: getComputedStyle(e).boxShadow,
          }),
        );
        if (rest === hover) throw Error("List hover missing");
        const resting = JSON.parse(rest),
          hovering = JSON.parse(hover);
        if (
          resting.background !== hovering.background ||
          resting.border !== hovering.border
        )
          throw Error("Hover changes row fill or border");
        const expectedShadow = await item.evaluate((e) => {
          const probe = document.createElement("div");
          probe.style.boxShadow = "var(--collection-shadow-hover)";
          e.append(probe);
          const shadow = getComputedStyle(probe).boxShadow;
          probe.remove();
          return shadow;
        });
        if (hovering.shadow !== expectedShadow)
          throw Error("Hover differs from grid shadow");
        await page.screenshot({
          path: path.join(out, `${theme}-${width}-${motion}-hover.png`),
        });
        if (JSON.stringify(before) !== JSON.stringify(after))
          throw Error("Hover moved row");
        if (
          await page
            .locator(".cask-table-scroll")
            .evaluate((e) => e.scrollWidth > e.clientWidth + 1)
        )
          throw Error("Overflow");
        const duration = await row.evaluate(
          (e) => getComputedStyle(e).transitionDuration,
        );
        if (
          motion === "reduce" &&
          duration.split(",").some((v) => parseFloat(v) > 0.01)
        )
          throw Error("Reduced motion");
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
          throw Error(
            JSON.stringify(
              violations.map((v) => ({
                id: v.id,
                nodes: v.nodes.map((n) => ({
                  target: n.target,
                  summary: n.failureSummary,
                })),
              })),
            ),
          );
        reports.push({ theme, width, motion, duration });
        await context.close();
      }
  await fs.writeFile(
    path.join(out, "result.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log("List and motion audit passed");
} finally {
  await browser.close();
  server.close();
}
