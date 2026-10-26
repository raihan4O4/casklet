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
  out = path.resolve(".jury/overview-row-border"),
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
    for (const width of [390, 560, 1024])
      for (const emptyContents of [true, false]) {
        const context = await browser.newContext({
          viewport: { width, height: 760 },
          reducedMotion: "reduce",
        });
        await context.addInitScript(installFixture, {
          theme,
          detail: true,
          reference: true,
          emptyContents,
        });
        await context.addInitScript(() => {
          localStorage.setItem("casklet.accent", "#000000");
          localStorage.setItem(
            "casklet.cask-appearance.v1",
            JSON.stringify({
              "qa-cask-0": { symbol: "icon:terminal", color: "#84cc16" },
            }),
          );
        });
        const page = await context.newPage(),
          errors = [];
        page.on("pageerror", (e) => errors.push(e.message));
        await page.goto(
          `http://127.0.0.1:${server.address().port}/?cask=qa-cask-0`,
        );
        await page.locator(".information-panel").scrollIntoViewIfNeeded();
        const rowGeometry = await page.evaluate(() => {
          const row = document
            .querySelector(".storage-refresh")
            .getBoundingClientRect();
          const label = document
            .querySelector(".storage-refresh span")
            .getBoundingClientRect();
          const glyph = document
            .querySelector(".storage-refresh svg")
            .getBoundingClientRect();
          const next = document
            .querySelector(".category-storage")
            .getBoundingClientRect();
          return {
            labelError: Math.abs(
              label.top + label.height / 2 - (row.top + next.top) / 2,
            ),
            iconError: Math.abs(
              glyph.top + glyph.height / 2 - (row.top + next.top) / 2,
            ),
            bottomBorder: parseFloat(
              getComputedStyle(document.querySelector(".information-panel"))
                .borderBottomWidth,
            ),
          };
        });
        if (
          rowGeometry.labelError > 1 ||
          rowGeometry.iconError > 1 ||
          rowGeometry.bottomBorder <= 0
        )
          throw Error("Refresh/border geometry " + JSON.stringify(rowGeometry));
        const dateInsets = await page.evaluate(() => {
          const card = document
            .querySelector(".information-panel")
            .getBoundingClientRect();
          const labels = [...document.querySelectorAll(".cask-dates dt")].map(
            (el) => el.getBoundingClientRect(),
          );
          const values = [...document.querySelectorAll(".cask-dates dd")].map(
            (el) => el.getBoundingClientRect(),
          );
          return {
            top: Math.min(...labels.map((r) => r.top)) - card.top,
            bottom: card.bottom - Math.max(...values.map((r) => r.bottom)),
          };
        });
        if (Math.abs(dateInsets.top - dateInsets.bottom) > 1)
          throw Error("Uneven date padding " + JSON.stringify(dateInsets));
        await page.screenshot({
          path: path.join(
            out,
            `${theme}-${width}-${emptyContents ? "empty" : "populated"}-storage.png`,
          ),
        });
        await page
          .getByRole("button", {
            name: "Browse files and folders",
            exact: true,
          })
          .waitFor();
        await page.evaluate(() => document.fonts.ready);
        const counts = await page
          .locator(".compact-stats strong")
          .allTextContents();
        if (
          JSON.stringify(counts) !==
          JSON.stringify(emptyContents ? ["0", "0", "0"] : ["6", "4", "2"])
        )
          throw Error("Wrong counts " + JSON.stringify(counts));
        const statsGeometry = await page.evaluate(() => {
          const stats = document
            .querySelector(".compact-stats")
            .getBoundingClientRect();
          const icon = document
            .querySelector(".compact-stats > div .overview-symbol")
            .getBoundingClientRect();
          const filesIcon = document
            .querySelector(".compact-stats > button .overview-symbol")
            .getBoundingClientRect();
          const title = document
            .querySelector(".overview-contents h2")
            .getBoundingClientRect();
          const storageIcon = document
            .querySelector(".storage-symbol")
            .getBoundingClientRect();
          return {
            glyphsCentered: [
              ...document.querySelectorAll(
                ".compact-stats .overview-symbol,.compact-stats .overview-open",
              ),
            ].every((el) => {
              const tile = el.getBoundingClientRect(),
                glyph = el.querySelector("svg").getBoundingClientRect();
              return (
                Math.abs(
                  tile.left + tile.width / 2 - glyph.left - glyph.width / 2,
                ) < 1 &&
                Math.abs(
                  tile.top + tile.height / 2 - glyph.top - glyph.height / 2,
                ) < 1
              );
            }),
            iconDelta: Math.abs(icon.left - filesIcon.left),
            panelInsetDelta: Math.abs(title.left - storageIcon.left),
            iconWidth: icon.width,
            clipped: [
              ...document.querySelectorAll(".compact-stats strong"),
            ].some((el) => {
              const r = el.getBoundingClientRect();
              return r.right > stats.right || r.left < stats.left;
            }),
          };
        });
        if (
          !statsGeometry.glyphsCentered ||
          statsGeometry.iconWidth > 40 ||
          statsGeometry.clipped ||
          statsGeometry.iconDelta > 1 ||
          statsGeometry.panelInsetDelta > 1
        )
          throw Error("Stats geometry " + JSON.stringify(statsGeometry));
        const overflow = await page.evaluate(
          () =>
            document.documentElement.scrollWidth > innerWidth ||
            document.querySelector(".workspace-content").scrollWidth >
              document.querySelector(".workspace-content").clientWidth + 1,
        );
        if (overflow) throw Error(`Overflow ${theme} ${width}`);
        await page.screenshot({
          path: path.join(
            out,
            `${theme}-${width}-${emptyContents ? "empty" : "populated"}.png`,
          ),
        });
        await page
          .getByRole("button", {
            name: "Browse files and folders",
            exact: true,
          })
          .click();
        await page
          .getByRole("button", { name: "Overview", exact: true })
          .click();
        await page
          .getByRole("button", { name: "Browse applications", exact: true })
          .click();
        await page
          .getByRole("button", { name: "Overview", exact: true })
          .click();
        await page
          .getByRole("button", { name: "Refresh sizes", exact: true })
          .click();
        await page
          .getByRole("button", { name: "Refresh sizes", exact: true })
          .waitFor();
        if (
          !(await page.evaluate(() =>
            window.__QA_CALLS__.some(
              (c) => c.command === "storage_summary" && c.args.refresh === true,
            ),
          ))
        )
          throw Error("Explicit size refresh not called");
        await page.locator(".category-storage summary").click();
        await page.locator(".category-storage summary").click();
        await page
          .getByRole("button", { name: "Cleanup", exact: true })
          .click();
        await page
          .getByRole("button", { name: "Overview", exact: true })
          .click();
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
          throw Error(
            JSON.stringify({ theme, width, emptyContents, violations, errors }),
          );
        reports.push({
          theme,
          width,
          emptyContents,
          counts,
          overflow,
          violations,
          errors,
        });
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
