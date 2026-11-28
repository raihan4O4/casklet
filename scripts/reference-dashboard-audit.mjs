import fs from "node:fs/promises";
import path from "node:path";
import http from "node:http";
import { createRequire } from "node:module";
import { installFixture } from "./ui-fixture.mjs";
const require = createRequire(
  path.resolve("target/ui-audit-tools/package.json"),
);
const { chromium } = require("playwright");
const dist = path.resolve("apps/desktop/dist");
const out = path.resolve(".jury/cask-card-muted");
await fs.mkdir(out, { recursive: true });
const server = http.createServer(async (req, res) => {
  try {
    const p = new URL(req.url, "http://localhost").pathname;
    const file = path.join(dist, p === "/" ? "index.html" : p);
    res.setHeader(
      "Content-Type",
      {
        ".js": "application/javascript",
        ".css": "text/css",
        ".html": "text/html",
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
const browser = await chromium.launch({ channel: "chrome", headless: true });
try {
  const context = await browser.newContext({
    viewport: { width: 1024, height: 720 },
    reducedMotion: "reduce",
  });
  await context.addInitScript(installFixture, {
    reference: true,
    incomplete: true,
  });
  await context.addInitScript(() => {
    localStorage.setItem("casklet.accent", "#000000");
    localStorage.setItem(
      "casklet.cask-appearance.v1",
      JSON.stringify(
        Object.fromEntries(
          [
            "icon:terminal",
            "icon:storage",
            "icon:terminal",
            "emoji:\u{1f3b9}",
            "icon:folder",
          ].map((symbol, i) => [
            `qa-cask-${i}`,
            {
              symbol,
              color: ["#3b82f6", "#fbbf24", "#fb4b85", "#a78bfa", "#14b8a6"][i],
            },
          ]),
        ),
      ),
    );
  });
  async function captureCardStates(page, theme) {
    const row = page.locator(".cask-row").nth(theme === "light" ? 0 : 1),
      card = row.locator("..");
    await page.mouse.move(10, 100);
    await page
      .locator(":focus")
      .evaluateAll((els) => els.forEach((el) => el.blur()));
    const initial = await card.evaluate((el) => ({
      border: getComputedStyle(el).borderColor,
      shadow: getComputedStyle(el).boxShadow,
    }));
    await page.screenshot({ path: path.join(out, `${theme}-default.png`) });
    await row.hover();
    const hover = await card.evaluate((el) => getComputedStyle(el).boxShadow);
    if (hover === initial.shadow) throw Error("Hover elevation missing");
    await page.screenshot({ path: path.join(out, `${theme}-hover.png`) });
    await row.click();
    await page.mouse.move(10, 100);
    await row.evaluate((el) => el.blur());
    const current = await card.evaluate((el) => ({
      border: getComputedStyle(el).borderColor,
      shadow: getComputedStyle(el).boxShadow,
    }));
    if (
      (await row.getAttribute("aria-current")) !== "true" ||
      initial.border !== current.border ||
      initial.shadow === current.shadow
    )
      throw Error(
        "Selected card did not retain neutral edge and distinct elevation",
      );
    await page.screenshot({ path: path.join(out, `${theme}-selected.png`) });
    await row.focus();
    await page.screenshot({ path: path.join(out, `${theme}-focus.png`) });
    await row.evaluate((el) => el.blur());
  }
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.getByRole("button", { name: "Open Files", exact: true }).waitFor();
  await page.evaluate(() => document.fonts.ready);
  const scaleReports = [];
  for (const section of ["Casks", "Installations", "Activity", "Settings"]) {
    await page.getByRole("button", { name: section, exact: true }).click();
    scaleReports.push(
      await page.evaluate((section) => {
        const style = (selector) => {
          const el = document.querySelector(selector),
            s = getComputedStyle(el),
            r = el.getBoundingClientRect();
          return {
            font: s.fontSize,
            line: s.lineHeight,
            height: r.height,
            padding: s.padding,
            zoom: s.zoom,
          };
        };
        return {
          section,
          nav: style(".app-navigation"),
          brand: style(".rail-brand"),
          label: style(".app-navigation nav button"),
          icon: style(".app-navigation .rail-icon svg"),
          heading: style(".page-intro h1"),
          search: ["Casks", "Installations"].includes(section)
            ? style(".search-field")
            : null,
          input: ["Casks", "Installations"].includes(section)
            ? style(".search-field input")
            : null,
          dpr: devicePixelRatio,
        };
      }, section),
    );
    await page.screenshot({
      path: path.join(out, `section-${section.toLowerCase()}.png`),
    });
  }
  for (const report of scaleReports.slice(1)) {
    for (const key of ["nav", "brand", "label", "icon", "heading", "dpr"])
      if (JSON.stringify(report[key]) !== JSON.stringify(scaleReports[0][key]))
        throw Error(
          `Section scale mismatch: ${report.section} ${key}: ${JSON.stringify(scaleReports)}`,
        );
  }
  const [casks, installed] = scaleReports;
  if (
    casks.search.height !== installed.search.height ||
    casks.input.font !== installed.input.font
  )
    throw Error("Search control scales differ");
  await fs.writeFile(
    path.join(out, "section-scale.json"),
    JSON.stringify(scaleReports, null, 2),
  );
  await page.getByRole("button", { name: "Casks", exact: true }).click();

  await page
    .locator(".window-body")
    .screenshot({ path: path.join(out, "light.png") });
  console.log(
    await page.evaluate(() =>
      Object.fromEntries(
        [".library-intro", ".cask-toolbar", ".table-heading", ".cask-row"].map(
          (s) => {
            const r = document.querySelector(s).getBoundingClientRect();
            return [
              s,
              { x: r.x, y: r.y - 32, width: r.width, height: r.height },
            ];
          },
        ),
      ),
    ),
  );
  await page
    .locator(".casks-stack")
    .screenshot({ path: path.join(out, "light-artwork.png") });
  await captureCardStates(page, "light");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("radio", { name: "Dark", exact: true }).check();
  await page.getByRole("button", { name: "Casks", exact: true }).click();
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
  await page.waitForTimeout(150);
  await page
    .locator(".window-body")
    .screenshot({ path: path.join(out, "dark.png") });
  await page
    .locator(".casks-stack")
    .screenshot({ path: path.join(out, "dark-artwork.png") });
  await captureCardStates(page, "dark");
  console.log(
    "dark action",
    await page.locator(".new-cask-button").evaluate((el) => ({
      color: getComputedStyle(el).color,
      background: getComputedStyle(el).backgroundColor,
      text: el.textContent,
      action: getComputedStyle(el).getPropertyValue("--action-text"),
    })),
  );
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("button", { name: "Blue", exact: true }).click();
  await page.getByRole("radio", { name: "Light", exact: true }).check();
  await page.getByRole("button", { name: "Casks", exact: true }).click();
  await page
    .locator(".window-body")
    .screenshot({ path: path.join(out, "blue.png") });
  if (
    (await page
      .locator(".new-cask-button")
      .evaluate((el) => getComputedStyle(el).backgroundColor)) !==
    "rgb(37, 99, 235)"
  )
    throw Error("Accent failed");
  await page.keyboard.press("Control+k");
  if (
    !(await page
      .getByRole("textbox", { name: "Search casks", exact: true })
      .evaluate((el) => el === document.activeElement))
  )
    throw Error("Shortcut failed");
  if (await page.getByRole("button", { name: /Actions for/ }).count())
    throw Error("Cask action menus remain");
  await page.getByRole("button", { name: "List view", exact: true }).click();
  if (await page.getByRole("button", { name: /Actions for/ }).count())
    throw Error("List action menus remain");
  await page.getByRole("button", { name: "Card view", exact: true }).click();
  for (const width of [1024, 768, 390]) {
    await page.setViewportSize({ width, height: 844 });
    await page
      .locator(".window-body")
      .screenshot({ path: path.join(out, `width-${width}.png`) });
    if (
      await page.evaluate(
        () =>
          document.querySelector(".main-content").scrollWidth >
          document.querySelector(".main-content").clientWidth,
      )
    )
      throw Error(`Overflow at ${width}`);
  }
  if (errors.length) throw Error(errors.join("\n"));
  for (const scale of [1, 1.25, 1.5]) {
    const dpiContext = await browser.newContext({
      viewport: { width: 1020, height: 708 },
      deviceScaleFactor: scale,
      reducedMotion: "reduce",
    });
    await dpiContext.addInitScript(installFixture, {
      reference: true,
      incomplete: true,
    });
    const dpiPage = await dpiContext.newPage();
    await dpiPage.goto(`http://127.0.0.1:${server.address().port}`);
    await dpiPage
      .getByRole("button", { name: "Open Files", exact: true })
      .waitFor();
    for (const view of ["grid", "list"]) {
      await dpiPage
        .getByRole("button", {
          name: view === "grid" ? "Card view" : "List view",
          exact: true,
        })
        .click();
      const problems = await dpiPage.evaluate(() => {
        const issues = [];
        const intro = document
          .querySelector(".library-intro")
          .getBoundingClientRect();
        for (const el of document.querySelectorAll(
          ".library-summary dt,.library-summary dd",
        )) {
          const r = el.getBoundingClientRect();
          if (
            el.scrollWidth > el.clientWidth + 1 ||
            r.right > intro.right ||
            r.bottom > intro.bottom
          )
            issues.push("Summary overflow");
        }
        for (const el of document.querySelectorAll(".table-heading button"))
          if (el.getBoundingClientRect().width > 110)
            issues.push("Stretched sort control");
        if (
          document.querySelector(".cask-table-scroll").getBoundingClientRect()
            .height < 300
        )
          issues.push("Insufficient collection viewport");
        return issues;
      });
      if (problems.length)
        throw Error(`DPI ${scale} ${view}: ${problems.join(", ")}`);
      await dpiPage.screenshot({
        path: path.join(out, `dpi-${scale}-${view}.png`),
      });
    }
    await dpiContext.close();
  }
  console.log("Theme/accent/shortcut/card-state checks passed");
} finally {
  await browser.close();
  server.close();
}
