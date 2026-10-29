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
  out = path.resolve(".jury/main-settings-search"),
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
    for (const width of [390, 1024]) {
      const context = await browser.newContext({
        viewport: { width, height: 900 },
        reducedMotion: "reduce",
      });
      await context.addInitScript(installFixture, { theme });
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", (e) => errors.push(e.message));
      await page.goto(`http://127.0.0.1:${server.address().port}/`);
      const style = () =>
        page
          .locator(".search-field")
          .first()
          .evaluate((e) => {
            const s = getComputedStyle(e);
            return {
              height: s.height,
              radius: s.borderRadius,
              padding: s.padding,
            };
          });
      const caskStyle = await style();
      await page
        .getByRole("button", { name: "Installations", exact: true })
        .click();
      const installStyle = await style();
      if (JSON.stringify(caskStyle) !== JSON.stringify(installStyle))
        throw Error(JSON.stringify({ caskStyle, installStyle }));
      await page.keyboard.press("Control+k");
      if (
        !(await page
          .getByRole("textbox", { name: "Search installed applications" })
          .evaluate((e) => e === document.activeElement))
      )
        throw Error("Search shortcut");
      await page.getByRole("button", { name: "Search", exact: true }).click();
      await page.screenshot({
        path: path.join(out, `${theme}-${width}-installations.png`),
      });
      await page.getByRole("button", { name: "Settings", exact: true }).click();
      if (
        await page
          .getByRole("button", { name: "Reset to default", exact: true })
          .count()
      )
        throw Error("Reset still exists");
      await page.getByRole("button", { name: "Iris", exact: true }).click();
      await page.reload();
      await page.getByRole("button", { name: "Settings", exact: true }).click();
      await page.getByRole("button", { name: "Iris", exact: true }).waitFor();
      if (
        (await page
          .getByRole("button", { name: "Iris", exact: true })
          .getAttribute("aria-pressed")) !== "true"
      )
        throw Error("Accent not persisted");
      await page.screenshot({
        path: path.join(out, `${theme}-${width}-settings.png`),
      });
      const accentGeometry=await page.locator('.accent-custom').evaluate(e=>{
        const picker=e.querySelector('.color-picker').getBoundingClientRect(),hex=e.querySelector('.accent-hex input').getBoundingClientRect();
        return {pickerHeight:picker.height,hexHeight:hex.height,centerError:Math.abs((picker.top+picker.height/2)-(hex.top+hex.height/2)),font:parseFloat(getComputedStyle(e.querySelector('.accent-hex input')).fontSize)};
      });
      if(accentGeometry.pickerHeight!==38 || accentGeometry.hexHeight!==38 || accentGeometry.centerError>1 || accentGeometry.font!==13)throw Error(JSON.stringify(accentGeometry));
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
      if (
        await page
          .locator(".main-content")
          .evaluate((e) => e.scrollWidth > e.clientWidth + 1)
      )
        throw Error("Overflow");
      if (errors.length) throw Error(errors.join());
      reports.push({ theme, width, caskStyle, installStyle });
      await context.close();
    }
  await fs.writeFile(
    path.join(out, "result.json"),
    JSON.stringify(reports, null, 2),
  );
  console.log("Main settings and search passed");
} finally {
  await browser.close();
  server.close();
}
