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
  out = path.resolve(".jury/selection-hover"),
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
      async function aligned(container, selector) {
        await page.waitForTimeout(250);
        const result = await page.locator(container).evaluate((e, selector) => {
          const a = e
              .querySelector(".selection-indicator")
              .getBoundingClientRect(),
            b = e.querySelector(selector).getBoundingClientRect();
          return Math.max(
            Math.abs(a.x - b.x),
            Math.abs(a.y - b.y),
            Math.abs(a.width - b.width),
            Math.abs(a.height - b.height),
          );
        }, selector);
        if (result > 1) throw Error("Indicator not aligned " + result);
      }
      async function sample(container, button) {
        return await page.locator(container).evaluate(async (e, name) => {
          const target = [...e.querySelectorAll("button")].find(
              (b) => b.textContent.trim() === name,
            ),
            indicator = e.querySelector(".selection-indicator");
          target.click();
          const samples = [];
          for (let i = 0; i < 18; i++) {
            await new Promise(requestAnimationFrame);
            samples.push(indicator.getBoundingClientRect().x);
          }
          return samples;
        }, button);
      }
      await aligned(".app-navigation nav", 'button[aria-current="page"]');
      const samples = await sample(".app-navigation nav", "Installations");
      if (
        motion === "no-preference" &&
        new Set(samples.map(Math.round)).size < 3
      )
        throw Error("Nav did not slide " + JSON.stringify(samples));
      if (motion === "reduce" && new Set(samples.map(Math.round)).size > 1)
        throw Error("Reduced nav moved");
      await aligned(".app-navigation nav", 'button[aria-current="page"]');
      await page.locator(".app-navigation nav").evaluate((e) => {
        const b = e.querySelectorAll("button");
        b[0].click();
        b[3].click();
        b[1].click();
      });
      await aligned(".app-navigation nav", 'button[aria-current="page"]');
      await page.locator(".app-navigation nav").evaluate(async (e) => {
        const b = e.querySelectorAll("button");
        b[0].click();
        await new Promise((r) => setTimeout(r, 45));
        b[3].click();
        await new Promise((r) => setTimeout(r, 45));
        b[1].click();
      });
      await aligned(".app-navigation nav", 'button[aria-current="page"]');
      await page.setViewportSize({ width: 560, height: 900 });
      await aligned(".app-navigation nav", 'button[aria-current="page"]');
      await page.screenshot({
        path: path.join(out, `${theme}-${motion}-main.png`),
      });
      await page.goto(
        `http://127.0.0.1:${server.address().port}/?cask=qa-cask-0`,
      );
      await page
        .getByRole("button", { name: "Overview", exact: true })
        .waitFor();
      await aligned(
        ".app-navigation.compact nav",
        'button[aria-current="page"]',
      );
      const tile = page.getByRole("button", {
        name: "Browse files and folders",
        exact: true,
      });
      const rect = await tile.boundingBox();
      await tile.hover();
      await page.waitForTimeout(200);
      if (
        (await tile.evaluate((e) => getComputedStyle(e).boxShadow)) === "none"
      )
        throw Error("Overview hover missing");
      if (JSON.stringify(rect) !== JSON.stringify(await tile.boundingBox()))
        throw Error("Hover moved layout");
      await page.screenshot({
        path: path.join(out, `${theme}-${motion}-overview.png`),
      });
      const sections = await sample(
        ".app-navigation.compact nav",
        "Applications",
      );
      if (
        motion === "no-preference" &&
        new Set(sections.map(Math.round)).size < 3
      )
        throw Error("Cask nav did not slide");
      await aligned(
        ".app-navigation.compact nav",
        'button[aria-current="page"]',
      );
      const filters = await sample(".application-filters", "References");
      if (
        motion === "no-preference" &&
        new Set(filters.map(Math.round)).size < 3
      )
        throw Error("Filter did not slide");
      await aligned(".application-filters", 'button[aria-pressed="true"]');
      await page.locator(".application-filters").evaluate((e) => {
        const b = e.querySelectorAll("button");
        b[0].click();
        b[1].click();
        b[2].click();
      });
      await aligned(".application-filters", 'button[aria-pressed="true"]');
      await page.screenshot({
        path: path.join(out, `${theme}-${motion}-filters.png`),
      });
      await page.keyboard.press("Alt+4");
      await aligned(
        ".app-navigation.compact nav",
        'button[aria-current="page"]',
      );
      const setting = page.locator('summary[aria-label="Categories"]');
      await setting.hover();
      await page.waitForTimeout(200);
      if (
        (await setting.evaluate(
          (e) => getComputedStyle(e.closest(".settings-card")).boxShadow,
        )) === "none"
      )
        throw Error("Settings hover missing");
      if (
        (await setting.evaluate((e) => getComputedStyle(e).boxShadow)) !==
        "none"
      )
        throw Error("Inset hover surface remains");
      const card = setting.locator("xpath=../..");
      await card.hover({ position: { x: 4, y: 4 } });
      await page.waitForTimeout(200);
      if (
        (await card.evaluate((e) => getComputedStyle(e).boxShadow)) === "none"
      )
        throw Error("Padding has no hover");
      await setting.click();
      await page
        .getByRole("textbox", { name: "New category", exact: true })
        .waitFor();
      await page.waitForTimeout(220);
      await page.screenshot({
        path: path.join(out, `${theme}-${motion}-settings.png`),
      });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.keyboard.press("Alt+1");
      await aligned(
        ".app-navigation.compact nav",
        'button[aria-current="page"]',
      );
      if (
        await page
          .locator(".selection-indicator")
          .evaluateAll((nodes) =>
            nodes.some((e) =>
              e.getAnimations().some((a) => a.playState === "running"),
            ),
          )
      )
        throw Error("Reduced indicator animating");
      if (errors.length) throw Error(errors.join());
      reports.push({ theme, motion, passed: true });
      await context.close();
    }
  await fs.writeFile(
    path.join(out, "result.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log("Selection and hover motion passed");
} finally {
  await browser.close();
  server.close();
}
