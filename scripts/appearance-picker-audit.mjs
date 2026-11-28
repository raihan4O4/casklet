import fs from "node:fs/promises";
import path from "node:path";
import http from "node:http";
import { createRequire } from "node:module";
import { installFixture } from "./ui-fixture.mjs";
const require = createRequire(
    path.resolve("target/ui-audit-tools/package.json"),
  ),
  { chromium } = require("playwright");
const axe = await fs.readFile(require.resolve("axe-core/axe.min.js"), "utf8"),
  out = path.resolve(".jury/appearance-settings-footer"),
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
        viewport: { width, height: 720 },
        reducedMotion: "reduce",
      });
      await context.addInitScript(installFixture, { theme, detail: true });
      const page = await context.newPage(),
        errors = [];
      page.on("pageerror", (e) => errors.push(e.message));
      await page.goto(
        `http://127.0.0.1:${server.address().port}/?cask=qa-cask-0`,
      );
      await page.getByRole("button", { name: "Settings", exact: true }).click();
      await page
        .getByRole("button", { name: "Change appearance", exact: true })
        .click();
      await page
        .getByRole("button", { name: "White color", exact: true })
        .click();
      if (
        (await page
          .locator(".appearance-trigger .white-background")
          .evaluate((el) => getComputedStyle(el).backgroundColor)) !==
        "rgb(255, 255, 255)"
      )
        throw Error("Preview is not white");
      if (
        (await page
          .getByRole("button", { name: "Save appearance", exact: true })
          .count()) ||
        (await page
          .getByRole("button", { name: "Cancel appearance", exact: true })
          .count())
      )
        throw Error("Local appearance actions remain");
      if (
        !(await page
          .getByRole("button", { name: "Save changes", exact: true })
          .isEnabled())
      )
        throw Error("Appearance does not enable Settings save");
      await page
        .getByRole("button", { name: "Reset changes", exact: true })
        .click();
      if (
        await page
          .getByRole("button", { name: "Save changes", exact: true })
          .isEnabled()
      )
        throw Error("Reset left Settings dirty");
      await page
        .getByRole("button", { name: "Change appearance", exact: true })
        .click();
      await page
        .getByRole("button", { name: "White color", exact: true })
        .click();
      for (const mode of ["icons", "emoji"]) {
        if (mode === "emoji") {
          await page
            .getByRole("button", { name: "Emoji", exact: true })
            .click();
          await page
            .getByRole("combobox", { name: "Emoji category", exact: true })
            .selectOption("People & Body");
        }
        await page
          .getByRole("button", { name: "Save changes", exact: true })
          .scrollIntoViewIfNeeded();
        for (const fraction of [0, 0.5, 1]) {
          await page
            .locator(".appearance-settings-editor > .cask-appearance-picker")
            .evaluate(
              (el, f) => (el.scrollTop = f * el.scrollHeight),
              fraction,
            );
          await page
            .locator(".appearance-symbol-options")
            .evaluate(
              (el, f) => (el.scrollTop = f * el.scrollHeight),
              fraction,
            );
          const m = await page.evaluate(() => {
            const picker = document
                .querySelector(
                  ".appearance-settings-editor > .cask-appearance-picker",
                )
                .getBoundingClientRect(),
              actions = document
                .querySelector(".workspace-footer")
                .getBoundingClientRect();
            return {
              pickerBottom: Math.min(
                picker.bottom,
                document
                  .querySelector(".workspace-content")
                  .getBoundingClientRect().bottom,
              ),
              actionsTop: actions.top,
              actionsBottom: actions.bottom,
              viewport: innerHeight,
              peek: !!document
                .elementFromPoint(actions.left + 40, actions.bottom + 4)
                ?.closest(".appearance-symbol-options"),
            };
          });
          if (m.pickerBottom > m.actionsTop + 1 || m.peek)
            throw Error("Symbols overlap actions " + JSON.stringify(m));
          reports.push({ theme, width, mode, fraction, ...m });
        }
        await page.screenshot({
          path: path.join(out, `${theme}-${width}-${mode}.png`),
        });
      }
      await page
        .getByRole("textbox", { name: "Search emojis", exact: true })
        .fill("🎨");
      await page
        .getByRole("combobox", { name: "Emoji category", exact: true })
        .selectOption("");
      await page
        .getByRole("button", { name: "Art emoji", exact: true })
        .click();
      await page
        .getByRole("button", { name: "Save changes", exact: true })
        .click();
      const saved = await page.evaluate(
        () =>
          JSON.parse(localStorage.getItem("casklet.cask-appearance.v1"))[
            "qa-cask-0"
          ],
      );
      if (saved.color !== "#ffffff" || saved.symbol !== "emoji:🎨")
        throw Error("White emoji not persisted");
      const badge = page.locator(".workspace-title .cask-badge");
      if (
        (await badge.evaluate((el) => getComputedStyle(el).backgroundColor)) !==
        "rgb(255, 255, 255)"
      )
        throw Error("Saved badge not white");
      await page.reload();
      await page
        .getByRole("button", { name: "Settings", exact: true })
        .waitFor();
      if (
        (await page
          .locator(".workspace-title .cask-badge")
          .evaluate((el) => getComputedStyle(el).backgroundColor)) !==
        "rgb(255, 255, 255)"
      )
        throw Error("White did not survive reload");
      await page.addScriptTag({ content: axe });
      const violations = (
        await page.evaluate(() =>
          window.axe.run(document, {
            runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21aa"] },
          }),
        )
      ).violations.map((v) => v.id);
      if (violations.length || errors.length)
        throw Error(JSON.stringify({ theme, width, violations, errors }));
      await context.close();
    }
  await fs.writeFile(
    path.join(out, "result.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log(JSON.stringify({ passed: true, checks: reports.length, out }));
} finally {
  await browser.close();
  server.close();
}
