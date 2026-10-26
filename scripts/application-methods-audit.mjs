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
  out = path.resolve(".jury/application-methods"),
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
    for (const width of [390, 560])
      for (const accent of ["#000000", "#7357d8", "#ffffff"]) {
        const context = await browser.newContext({
          viewport: { width, height: 760 },
          reducedMotion: "reduce",
        });
        await context.addInitScript(installFixture, { theme, detail: true });
        await context.addInitScript(
          (value) => localStorage.setItem("casklet.accent", value),
          accent,
        );
        const page = await context.newPage(),
          errors = [];
        page.on("pageerror", (e) => errors.push(e.message));
        await page.goto(
          `http://127.0.0.1:${server.address().port}/?cask=qa-cask-0`,
        );
        await page
          .getByRole("button", { name: "Applications", exact: true })
          .click();
        await page
          .getByRole("button", { name: "Add Application", exact: true })
          .click();
        if (
          (await page
            .locator(".application-add-picker > .workflow-options > button")
            .count()) !== 3
        )
          throw Error("Primary method count");
        const geometry = await page.evaluate(() => {
          const probe = document.createElement("span");
          probe.style.color = "var(--accent)";
          document.body.append(probe);
          const accent = getComputedStyle(probe).color;
          probe.remove();
          const rows = [
            ...document.querySelectorAll(
              ".application-add-picker > .workflow-options > button",
            ),
          ];
          return {
            accent,
            wrongColor: rows.some((row) =>
              [...row.querySelectorAll("svg")].some(
                (svg) => getComputedStyle(svg).color !== accent,
              ),
            ),
            offCenter: rows.some((row) =>
              [
                ...row.querySelectorAll(
                  ".application-method-icon,.application-method-chevron",
                ),
              ].some((tile) => {
                const r = tile.getBoundingClientRect(),
                  g = tile.querySelector("svg").getBoundingClientRect();
                return (
                  Math.abs(r.x + r.width / 2 - g.x - g.width / 2) > 1 ||
                  Math.abs(r.y + r.height / 2 - g.y - g.height / 2) > 1
                );
              }),
            ),
            overflow:
              document.querySelector(".workspace-content").scrollWidth >
              document.querySelector(".workspace-content").clientWidth + 1,
          };
        });
        if (geometry.wrongColor || geometry.offCenter || geometry.overflow)
          throw Error(JSON.stringify({ theme, width, accent, geometry }));
        const name = `${theme}-${width}-${accent.slice(1)}`;
        await page.screenshot({ path: path.join(out, name + ".png") });
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
        ).violations.map((v) => ({
          id: v.id,
          targets: v.nodes.map((n) => n.target),
        }));
        if (violations.length || errors.length)
          throw Error(JSON.stringify({ violations, errors }));
        for (const [index, title] of [
          "Install Application",
          "Associate existing app",
          "Install portable ZIP",
          "Install with WinGet",
        ].entries()) {
          if (index)
            await page
              .getByRole("button", { name: "Add Application", exact: true })
              .click();
          if (index === 3)
            await page.locator(".application-more-methods > summary").click();
          await page
            .locator(".application-add-picker button")
            .filter({ has: page.getByText(title, { exact: true }) })
            .click();
          await page.locator(".application-workflow-panel").waitFor();
          if (
            (await page
              .locator(".application-workflow-panel > .section-heading h3")
              .textContent()) !== title
          )
            throw Error("Wrong workflow");
          await page
            .getByRole("button", {
              name: "Close application workflow",
              exact: true,
            })
            .click();
        }
        reports.push({ theme, width, accent, ...geometry, violations, errors });
        await context.close();
      }
  await fs.writeFile(
    path.join(out, "result.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log(JSON.stringify({ passed: true, states: reports.length, out }));
} finally {
  await browser.close();
  server.close();
}
