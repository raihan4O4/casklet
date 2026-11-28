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
  out = path.resolve(".jury/cask-accent-roles"),
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
    for (const accent of [
      "#7357d8",
      "#448348",
      "#e5a521",
      "#000000",
      "#ffffff",
    ]) {
      const context = await browser.newContext({
        viewport: { width: 560, height: 900 },
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
      const token = async (name) =>
        page.evaluate((name) => {
          const e = document.createElement("span");
          e.style.color =
            name === "--danger-text"
              ? "var(--danger-text,#b93838)"
              : `var(${name})`;
          document.body.append(e);
          const value = getComputedStyle(e).color;
          e.remove();
          return value;
        }, name);
      const ink = await token("--accent"),
        surface = await token("--accent-surface"),
        neutral = await token("--text-heading");
      async function color(selector, expected, property = "color") {
        const actual = await page
          .locator(selector)
          .first()
          .evaluate((e, property) => getComputedStyle(e)[property], property);
        if (actual !== expected)
          throw Error(
            JSON.stringify({ selector, actual, expected, theme, accent }),
          );
      }
      await color(".app-navigation button.active", ink);
      await color(".app-navigation button.active svg", ink);
      await color(".overview-symbol", neutral);
      await page
        .getByRole("button", { name: "Browse applications", exact: true })
        .hover();
      await color(
        'button[aria-label="Browse applications"] .overview-open',
        ink,
      );
      await color(
        ".workspace-footer .danger-button",
        await token("--danger-text"),
      );
      await page
        .getByRole("button", { name: "Files & Folders", exact: true })
        .click();
      await page.locator(".resource-category summary").first().click();
      await color(".resource-category[open] .category-chevron", ink);
      await color(".files-inventory .category-marker", neutral);
      await page
        .getByRole("button", { name: "Applications", exact: true })
        .click();
      await color(".application-filters button.chosen", ink);
      await color(
        ".application-filters .selection-indicator",
        surface,
        "backgroundColor",
      );
      await color(
        ".application-row-actions button svg",
        await page
          .locator(".application-row-actions button")
          .first()
          .evaluate((e) => getComputedStyle(e).color),
      );
      await color(
        ".applications-heading .primary svg",
        await token("--action-text"),
      );
      await page
        .getByRole("button", { name: "Add Application", exact: true })
        .click();
      await color(".application-method-icon", ink);
      await page.screenshot({
        path: path.join(out, `${theme}-${accent.slice(1)}-apps.png`),
      });
      await page.getByRole("button", { name: "Settings", exact: true }).click();
      await page.locator('summary[aria-label="Cask settings"]').click();
      await color(
        ".settings-disclosure[open] .settings-disclosure-chevron",
        ink,
      );
      await color(".setting-symbol", neutral);
      await color(
        'input[type="checkbox"]',
        await token("--control-accent"),
        "accentColor",
      );
      await page
        .getByRole("textbox", { name: "Cask name", exact: true })
        .fill("Changed");
      await color(
        ".workspace-footer .primary-button",
        await token("--action-text"),
      );
      await page
        .getByRole("button", { name: "Change appearance", exact: true })
        .click();
      await page
        .getByRole("button", { name: "Terminal icon", exact: true })
        .click();
      await color(
        '.appearance-symbol-options button[aria-pressed="true"]',
        ink,
      );
      await page.screenshot({
        path: path.join(out, `${theme}-${accent.slice(1)}-settings.png`),
      });
      await page.addScriptTag({ content: axe });
      const violations = (
        await page.evaluate(() =>
          window.axe.run(document, {
            runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21aa"] },
          }),
        )
      ).violations;
      if (violations.length)
        throw Error(
          JSON.stringify(
            violations.map((v) => ({
              id: v.id,
              targets: v.nodes.map((n) => n.target),
            })),
          ),
        );
      if (errors.length) throw Error(errors.join());
      reports.push({ theme, accent, ink, passed: true });
      await context.close();
    }
  await fs.writeFile(
    path.join(out, "result.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log("Cask accent roles passed");
} finally {
  await browser.close();
  server.close();
}
