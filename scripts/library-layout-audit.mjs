// Production layout regression: intrinsic header, bounded library and internal scrolling.
// node scripts/library-layout-audit.mjs --tools target/ui-audit-tools --out .jury/library-layout
import fs from "node:fs/promises";
import path from "node:path";
import http from "node:http";
import { createRequire } from "node:module";
import { installFixture } from "./ui-fixture.mjs";

const arg = (name, fallback) =>
  process.argv.includes(name)
    ? process.argv[process.argv.indexOf(name) + 1]
    : fallback;
const require = createRequire(
  path.resolve(arg("--tools", "target/ui-audit-tools"), "package.json"),
);
const { chromium } = require("playwright");
const axe = await fs.readFile(require.resolve("axe-core/axe.min.js"), "utf8");
const out = path.resolve(arg("--out", ".jury/library-layout"));
await fs.mkdir(out, { recursive: true });
const dist = path.resolve("apps/desktop/dist");
const types = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".png": "image/png",
  ".woff2": "font/woff2",
};
const server = http.createServer(async (request, response) => {
  const pathname = new URL(request.url, "http://localhost").pathname;
  if (pathname === "/favicon.ico") return response.writeHead(204).end();
  const file = path.resolve(
    dist,
    pathname === "/" ? "index.html" : "." + decodeURIComponent(pathname),
  );
  if (!file.startsWith(dist + path.sep)) return response.writeHead(403).end();
  try {
    response
      .writeHead(200, {
        "content-type": types[path.extname(file)] ?? "application/octet-stream",
      })
      .end(await fs.readFile(file));
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const browser = await chromium.launch({ channel: "chrome", headless: true });
const reports = [];
const assert = (condition, message) => {
  if (!condition) throw Error(message);
};
async function geometry(page) {
  return page.evaluate(() => {
    const rect = (selector) => {
      const r = document.querySelector(selector).getBoundingClientRect();
      return {
        top: r.top,
        bottom: r.bottom,
        height: r.height,
        left: r.left,
        right: r.right,
      };
    };
    const main = document.querySelector(".main-content"),
      scroll = document.querySelector(".cask-table-scroll");
    return {
      header: rect(".library-intro"),
      toolbar: rect(".cask-toolbar"),
      search: rect(".cask-toolbar .search-field"),
      create: rect(".cask-toolbar .new-cask-button"),
      sort: rect(".table-heading"),
      scroll: rect(".cask-table-scroll"),
      main: rect(".main-content"),
      bottomPadding: parseFloat(getComputedStyle(main).paddingBottom),
      outerOverflow:
        main.scrollHeight > main.clientHeight + 1 ||
        document.documentElement.scrollWidth > innerWidth,
      horizontalOverflow: scroll.scrollWidth > scroll.clientWidth + 1,
      scrollTop: scroll.scrollTop,
      scrollHeight: scroll.scrollHeight,
      clientHeight: scroll.clientHeight,
      cards: [...document.querySelectorAll(".cask-row")].map((el) => {
        const r = el.getBoundingClientRect();
        return { top: r.top, bottom: r.bottom };
      }),
    };
  });
}
try {
  for (const theme of ["light", "dark"]) {
    for (const viewport of [
      { width: 1024, height: 720 },
      { width: 1024, height: 600 },
      { width: 1440, height: 900 },
      { width: 768, height: 720 },
      { width: 390, height: 844 },
    ]) {
      for (const count of [0, 4, 40]) {
        const context = await browser.newContext({
          viewport,
          reducedMotion: "reduce",
        });
        await context.addInitScript(installFixture, {
          theme,
          libraryCount: count,
        });
        const page = await context.newPage();
        const errors = [];
        page.on("pageerror", (error) => errors.push(error.message));
        page.on("requestfailed", (request) =>
          errors.push(request.failure()?.errorText),
        );
        await page.goto(`http://127.0.0.1:${server.address().port}`);
        await page
          .getByRole(count ? "button" : "heading", {
            name: count ? "Open Photography Course" : "No Casks yet",
            exact: true,
          })
          .waitFor();
        await page.evaluate(() => document.fonts.ready);
        for (const view of ["grid", "list"]) {
          await page
            .getByRole("button", {
              name: view === "grid" ? "Card view" : "List view",
              exact: true,
            })
            .click();
          await page.locator(".cask-table-scroll").evaluate((el) => {
            el.scrollTop = 0;
          });
          const name = `${theme}-${viewport.width}x${viewport.height}-${count}-${view}`;
          const start = await geometry(page);
          assert(
            start.create.left >= start.search.right &&
              Math.abs(start.create.top - start.search.top) <= 1 &&
              Math.abs(start.create.height - start.search.height) <= 1,
            name + ": New Cask is not aligned to the right of search",
          );
          assert(
            !start.outerOverflow && !start.horizontalOverflow,
            name + ": outer/horizontal overflow",
          );
          assert(
            start.header.bottom <= start.toolbar.top + 1 &&
              start.toolbar.bottom <= start.sort.top + 1 &&
              start.sort.bottom <= start.scroll.top + 1,
            name + ": controls overlap",
          );
          assert(
            Math.abs(
              start.scroll.bottom - (start.main.bottom - start.bottomPadding),
            ) <= 1,
            name + ": library does not fill remaining space",
          );
          assert(start.scroll.height > 0, name + ": zero-height library");
          if (
            viewport.width === 1024 &&
            viewport.height === 720 &&
            count === 4 &&
            view === "grid"
          ) {
            assert(
              start.scroll.height >= 320,
              name + ": insufficient two-row viewport " + JSON.stringify(start),
            );
            assert(
              start.cards.every(
                (card) =>
                  card.top >= start.scroll.top - 1 &&
                  card.bottom <= start.scroll.bottom + 1,
              ),
              name + ": four ordinary cards clipped",
            );
          }
          if (count === 40) {
            await page.locator(".cask-table-scroll").evaluate((el) => {
              el.scrollTop = el.scrollHeight;
            });
            const end = await geometry(page);
            assert(
              end.scrollTop > 0 &&
                Math.abs(end.header.top - start.header.top) <= 1,
              name + ": library scroll moved header",
            );
            assert(
              end.cards.at(-1).bottom <= end.scroll.bottom + 1,
              name + ": final card inaccessible",
            );
            if (viewport.width === 1024 && viewport.height === 720) {
              await page.setViewportSize({ width: 1024, height: 920 });
              const tall = await geometry(page);
              assert(
                Math.abs(tall.scroll.height - start.scroll.height - 200) <= 1,
                name + ": viewport height does not grow with window",
              );
              await page.setViewportSize(viewport);
            }
          }
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
          ).violations.map((item) => ({
            id: item.id,
            nodes: item.nodes.map((n) => ({
              target: n.target,
              summary: n.failureSummary,
            })),
          }));
          assert(
            !violations.length && !errors.length,
            name +
              ": accessibility/console failure " +
              JSON.stringify({ violations, errors }),
          );
          await page.screenshot({ path: path.join(out, name + ".png") });
          reports.push({
            name,
            theme,
            viewport,
            count,
            view,
            ...start,
            violations,
            errors: [...errors],
          });
        }
        await context.close();
      }
    }
  }
  await fs.writeFile(
    path.join(out, "report.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log(JSON.stringify({ passed: true, states: reports.length, out }));
} finally {
  await browser.close();
  await new Promise((resolve) => server.close(resolve));
}
