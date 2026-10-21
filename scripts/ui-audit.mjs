// Production-render audit at the app's actual desktop window sizes.
// Usage: node scripts/ui-audit.mjs --tools <external npm cache> --out .jury/desktop/pass-1
import fs from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import http from "node:http";
import { installFixture } from "./ui-fixture.mjs";

const arg = (name, fallback) =>
  process.argv[process.argv.indexOf(name) + 1] ?? fallback;
if (!process.argv.includes("--tools"))
  throw Error("Pass --tools with an external Playwright/axe cache");
const require = createRequire(path.resolve(arg("--tools"), "package.json"));
const { chromium } = require("playwright");
const axeSource = await fs.readFile(
  require.resolve("axe-core/axe.min.js"),
  "utf8",
);
const out = path.resolve(
  process.argv.includes("--out") ? arg("--out") : ".jury/desktop/pass-1",
);
await fs.mkdir(out, { recursive: true });
const dist = path.resolve("apps/desktop/dist");
const types = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".png": "image/png",
  ".ttf": "font/ttf",
  ".woff2": "font/woff2",
};
const server = http.createServer(async (request, response) => {
  // Chromium requests its own tab favicon; the desktop WebView has no tab.
  if (request.url === "/favicon.ico") {
    response.writeHead(204).end();
    return;
  }
  const pathname = decodeURIComponent(
    new URL(request.url, "http://localhost").pathname,
  );
  const file = path.resolve(
    dist,
    pathname === "/" ? "index.html" : "." + pathname,
  );
  if (file !== dist && !file.startsWith(dist + path.sep)) {
    response.writeHead(403).end();
    return;
  }
  try {
    const contents = await fs.readFile(file);
    response
      .writeHead(200, {
        "content-type": types[path.extname(file)] ?? "application/octet-stream",
      })
      .end(contents);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ channel: "chrome", headless: true });
const reports = [];
let current;
let page;
let errors;
let pageFailures;
const contexts = [];
async function start(theme, options = {}) {
  current = await browser.newContext({
    viewport:
      options.viewport ??
      (options.detail
        ? { width: 560, height: 760 }
        : { width: 1024, height: 720 }),
    reducedMotion: options.reduced ? "reduce" : "no-preference",
    deviceScaleFactor: options.scale ?? 1,
  });
  contexts.push(current);
  await current.addInitScript(installFixture, { ...options, theme });
  await current.addInitScript(() => {
    window.__QA_PERF__ = { lcpMs: null, cls: 0 };
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries())
        window.__QA_PERF__.lcpMs = entry.startTime;
    }).observe({ type: "largest-contentful-paint", buffered: true });
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries())
        if (!entry.hadRecentInput) window.__QA_PERF__.cls += entry.value;
    }).observe({ type: "layout-shift", buffered: true });
  });
  page = await current.newPage();
  if (options.throttle)
    await (
      await current.newCDPSession(page)
    ).send("Emulation.setCPUThrottlingRate", { rate: 4 });
  errors = [];
  pageFailures = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  page.on("requestfailed", (request) => pageFailures.push(request.url()));
  await page.goto(origin + (options.detail ? "/?cask=qa-cask-0" : "/"));
  await page.locator("h1").waitFor();
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(350);
}
async function shot(name, { audit = true } = {}) {
  await page.waitForTimeout(220);
  await page.screenshot({ path: path.join(out, name + ".png") });
  const geometry = await page.evaluate(() => ({
    viewport: {
      width: innerWidth,
      height: innerHeight,
      pixelRatio: devicePixelRatio,
    },
    overflowX: document.documentElement.scrollWidth > innerWidth,
    paint: window.__QA_PERF__,
    reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
    runningAnimations: document
      .getAnimations()
      .filter((a) => a.playState === "running").length,
    heading: document.querySelector("h1")?.textContent.trim(),
    contentScroll: [
      ...document.querySelectorAll(
        ".main-content,.workspace-content,.cask-table-scroll",
      ),
    ].map((el) => ({
      class: el.className,
      width: el.clientWidth,
      scrollWidth: el.scrollWidth,
      height: el.clientHeight,
      scrollHeight: el.scrollHeight,
    })),
    resources: performance
      .getEntriesByType("resource")
      .map(({ name, transferSize }) => ({
        name: name.split("/").pop(),
        transferSize,
      })),
  }));
  let violations = [];
  if (audit) {
    await page.addScriptTag({ content: axeSource });
    const result = await page.evaluate(async () =>
      window.axe.run(document, {
        runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21aa"] },
      }),
    );
    violations = result.violations.map(
      ({ id, impact, description, nodes }) => ({
        id,
        impact,
        description,
        nodes: nodes.map(({ target, failureSummary }) => ({
          target,
          failureSummary,
        })),
      }),
    );
  }
  reports.push({
    name,
    ...geometry,
    errors: [...errors],
    requestFailures: [...pageFailures],
    violations,
  });
  await fs.writeFile(
    path.join(out, "report.json"),
    JSON.stringify(reports, null, 2),
  );
}
try {
  for (const theme of ["light", "dark"]) {
    await start(theme);
    await shot(theme + "-inventory");
    await page.getByRole("button", { name: "List view", exact: true }).click();
    await shot(theme + "-list-view");
    await page.getByRole("button", { name: "Card view", exact: true }).click();
    const row = page.getByRole("button", { name: "Open Photography Course" });
    await row.hover();
    await shot(theme + "-row-hover", { audit: false });
    await row.focus();
    await page.keyboard.press("Tab");
    await page.keyboard.press("Shift+Tab");
    await shot(theme + "-row-focus", { audit: false });
    await page.getByRole("button", { name: "Items", exact: true }).click();
    const sorted = await page
      .getByRole("button", { name: "Items", exact: true })
      .getAttribute("data-direction");
    if (sorted !== "ascending") throw Error("Sort did not switch to ascending");
    await page.getByRole("button", { name: "Items", exact: true }).click();
    if (
      (await page
        .getByRole("button", { name: "Items", exact: true })
        .getAttribute("data-direction")) !== "descending"
    )
      throw Error("Sort did not toggle");
    await page
      .getByRole("textbox", { name: "Search casks", exact: true })
      .fill("No matching project");
    await shot(theme + "-search-empty");
    await page
      .getByRole("textbox", { name: "Search casks", exact: true })
      .fill("");
    await page.getByRole("button", { name: "New Cask", exact: true }).click();
    await shot(theme + "-create");
    await page.keyboard.press("Escape");
    await page.getByRole("dialog").waitFor({ state: "hidden" });
    if (
      !(await page
        .getByRole("button", { name: "New Cask", exact: true })
        .evaluate((el) => el === document.activeElement))
    )
      throw Error("Creation dialog did not restore focus");
    await page.getByRole("button", { name: "Activity", exact: true }).click();
    await shot(theme + "-activity");
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await shot(theme + "-settings");
    await page
      .getByRole("radio", {
        name: theme === "light" ? "Dark" : "Light",
        exact: true,
      })
      .check();
    if ((await page.locator("html").getAttribute("data-theme")) === theme)
      throw Error("Theme did not switch");
    await page
      .getByRole("radio", {
        name: theme === "light" ? "Light" : "Dark",
        exact: true,
      })
      .check();
    await page
      .getByRole("button", { name: "Installations", exact: true })
      .click();
    await shot(theme + "-installations");
    await start(theme, { detail: true });
    await shot(theme + "-overview");
    await page
      .getByRole("button", { name: "Browse files and folders", exact: true })
      .click();
    await page
      .getByRole("region", { name: "Resource inventory", exact: true })
      .waitFor();
    await page.getByRole("button", { name: "Overview", exact: true }).click();
    await page
      .getByRole("button", { name: "Cask actions", exact: true })
      .click();
    await shot(theme + "-workspace-menu");
    await page.keyboard.press("Escape");
    await page.getByRole("menu").waitFor({ state: "hidden" });
    if (await page.getByRole("menu").count())
      throw Error("Menu did not close on Escape");
    if (
      !(await page
        .getByRole("button", { name: "Cask actions", exact: true })
        .evaluate((el) => el === document.activeElement))
    )
      throw Error("Workspace menu did not restore focus");
    await page
      .getByRole("button", { name: "Files & Folders", exact: true })
      .click();
    await shot(theme + "-files");
    await page
      .getByRole("button", {
        name: "Details for Course notes.pdf",
        exact: true,
      })
      .click();
    await shot(theme + "-resource-details");
    await page
      .getByRole("button", { name: "Change Category", exact: true })
      .click();
    await shot(theme + "-category-editor");
    await page.keyboard.press("Escape");
    if (
      !(await page
        .getByRole("button", { name: "Change Category", exact: true })
        .evaluate((el) => el === document.activeElement))
    )
      throw Error("Category Escape did not restore focus");
    await page
      .getByRole("button", { name: "Change ownership", exact: true })
      .click();
    await page.getByRole("radio", { name: /^Owned by Cask/ }).check();
    await shot(theme + "-ownership-editor");
    if (
      await page
        .getByRole("button", { name: "Save ownership", exact: true })
        .isEnabled()
    )
      throw Error("Ownership save did not require consent");
    await page.keyboard.press("Escape");
    if (
      !(await page
        .getByRole("button", { name: "Change ownership", exact: true })
        .evaluate((el) => el === document.activeElement))
    )
      throw Error("Ownership Escape did not restore focus");
    await page
      .getByRole("button", {
        name: "Details for Editing references",
        exact: true,
      })
      .click();
    await page.getByText("reference.jpg", { exact: true }).waitFor();
    await shot(theme + "-folder-details");
    await page.getByRole("button", { name: "Add Items", exact: true }).click();
    await shot(theme + "-add-items");
    await page.getByRole("button", { name: "Add File", exact: true }).click();
    await shot(theme + "-resource-mode");
    await page
      .getByRole("radio", {
        name: "Reference at current location",
        exact: true,
      })
      .check();
    await shot(theme + "-reference-choice");
    await page
      .getByRole("radio", { name: "Manage at current location", exact: true })
      .check();
    await shot(theme + "-ownership-consent");
    await page
      .getByRole("button", { name: "Close resource intake", exact: true })
      .focus();
    await page.keyboard.press("Enter");
    await page
      .getByRole("region", { name: "Resource inventory", exact: true })
      .waitFor();
    await page
      .getByRole("button", { name: "Applications", exact: true })
      .click();
    await shot(theme + "-applications");
    await page
      .getByRole("button", { name: "Add Application", exact: true })
      .click();
    await shot(theme + "-application-picker");
    for (const [name, suffix] of [
      ["Install Application", "installer"],
      ["Associate existing app", "associate"],
      ["Install portable ZIP", "portable"],
      ["Install with WinGet", "winget"],
    ]) {
      await page.getByRole("button", { name: new RegExp("^" + name) }).click();
      await shot(theme + "-" + suffix);
      await page
        .getByRole("button", {
          name: "Close application workflow",
          exact: true,
        })
        .click();
      if (suffix !== "winget")
        await page
          .getByRole("button", { name: "Add Application", exact: true })
          .click();
    }
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await shot(theme + "-cask-settings");
    await page
      .getByLabel("New category", { exact: true })
      .fill("Field research");
    await shot(theme + "-category-settings");
    await page
      .getByRole("button", { name: "Create category", exact: true })
      .click();
    await page
      .getByRole("list", { name: "Available categories" })
      .getByText("Field research", { exact: true })
      .waitFor();
    await page
      .getByRole("region", { name: "Export Cask", exact: true })
      .scrollIntoViewIfNeeded();
    await page.getByText("Machine-specific export", { exact: true }).click();
    await shot(theme + "-export-settings");
    await page.getByRole("button", { name: "Overview", exact: true }).click();
    await page.getByRole("button", { name: "Cleanup", exact: true }).click();
    await shot(theme + "-cleanup");
    await page
      .getByLabel("Cleanup scope", { exact: true })
      .selectOption("documents");
    await page
      .getByRole("button", { name: "Create cleanup plan", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Review Cleanup", exact: true })
      .waitFor();
    await shot(theme + "-cleanup-plan");
    await page
      .getByRole("button", { name: "Review Cleanup", exact: true })
      .click();
    await shot(theme + "-cleanup-reviewed");
    if (
      await page
        .getByRole("button", { name: "Approve Cleanup", exact: true })
        .isEnabled()
    )
      throw Error("Review-only cleanup became enabled");
  }
  await start("light", { detail: true, removalEnabled: true });
  await page.getByRole("button", { name: "Cleanup", exact: true }).click();
  await page
    .getByLabel("Cleanup scope", { exact: true })
    .selectOption("documents");
  await page
    .getByRole("button", { name: "Create cleanup plan", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Review Cleanup", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Approve Cleanup", exact: true })
    .click();
  await shot("light-capable-cleanup-confirmation");
  if (
    await page
      .getByRole("button", { name: "Execute Cleanup", exact: true })
      .isEnabled()
  )
    throw Error("Cleanup execution did not require confirmation");
  await start("light", { empty: true });
  await shot("light-empty");
  await start("dark", { failed: true });
  await shot("dark-library-error");
  await start("light", { long: true, scale: 1.25 });
  await shot("light-long-125-percent");
  await start("light", { throttle: true });
  await shot("light-cpu4-paint");
  await start("dark", { detail: true, long: true, reduced: true });
  await shot("dark-long-reduced-motion");
  for (const width of [768, 390]) {
    await start("light", { viewport: { width, height: 844 } });
    await shot(`light-library-${width}`);
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await shot(`light-settings-${width}`);
    await start("dark", {
      detail: true,
      viewport: { width: Math.min(width, 560), height: 844 },
    });
    await shot(`dark-workspace-${Math.min(width, 560)}`);
    if (width === 390) {
      await page
        .getByRole("button", { name: "Files & Folders", exact: true })
        .click();
      await page
        .getByRole("button", {
          name: "Details for Course notes.pdf",
          exact: true,
        })
        .click();
      await page
        .getByRole("button", { name: "Change ownership", exact: true })
        .click();
      await page.getByRole("radio", { name: /^Owned by Cask/ }).check();
      await shot("dark-ownership-editor-390");
      await page.keyboard.press("Escape");
      await page
        .getByRole("button", { name: "Change Category", exact: true })
        .click();
      await shot("dark-category-editor-390");
      await page.keyboard.press("Escape");
    }
    await page
      .getByRole("button", { name: "Applications", exact: true })
      .click();
    await shot(`dark-applications-${Math.min(width, 560)}`);
    await page.getByRole("button", { name: "Cleanup", exact: true }).click();
    await shot(`dark-cleanup-${Math.min(width, 560)}`);
  }
  await start("light", { reduced: true });
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await shot("light-keyboard-skip", { audit: false });
  const keyboardStops = [];
  for (let step = 0; step < 18; step++) {
    await page.keyboard.press("Tab");
    keyboardStops.push(
      await page.evaluate(() => {
        const el = document.activeElement;
        const style = getComputedStyle(el);
        const box = el.getBoundingClientRect();
        const field = el.closest(".search-field");
        return {
          tag: el.tagName,
          name: el.getAttribute("aria-label") ?? el.textContent.trim(),
          outline: style.outline,
          visible:
            box.width > 0 &&
            box.height > 0 &&
            box.top >= 0 &&
            box.bottom <= innerHeight,
          hasFocusTreatment:
            parseFloat(style.outlineWidth) > 0 ||
            style.boxShadow !== "none" ||
            !!field,
        };
      }),
    );
  }
  const summary = {
    scope:
      "Production bundle, test-only contract-shaped IPC fixture, fixed desktop viewports. Not a native filesystem/provider/installer test. No mobile or SEO score applies.",
    screenshots: reports.length,
    keyboardStops,
    consoleErrors: reports.flatMap((r) => r.errors),
    requestFailures: reports.flatMap((r) => r.requestFailures),
    overflow: reports.filter(
      (r) =>
        r.overflowX || r.contentScroll.some((s) => s.scrollWidth > s.width),
    ),
    violations: reports
      .filter((r) => r.violations.length)
      .map((r) => ({ name: r.name, violations: r.violations })),
    checked: [
      "sort toggle",
      "search",
      "outside layout dialogs",
      "theme switching",
      "native-workspace presentation",
      "reduced motion",
      "125% pixel scale",
      "long text",
      "collection card/list switch",
      "Overview shortcuts",
      "dialog/menu Escape focus restoration",
      "keyboard intake close",
      "all application-add methods",
      "reference/ownership consent",
      "cleanup review/approval/confirmation gates",
      "390/768 browser reflow",
    ],
  };
  await fs.writeFile(
    path.join(out, "summary.json"),
    JSON.stringify(summary, null, 2),
  );
  await fs.writeFile(
    path.join(out, "audit.txt"),
    JSON.stringify(summary, null, 2),
  );
  console.log(
    JSON.stringify({
      out,
      screenshots: summary.screenshots,
      consoleErrors: summary.consoleErrors.length,
      overflow: summary.overflow.length,
      screensWithViolations: summary.violations.length,
    }),
  );
  if (
    summary.consoleErrors.length ||
    summary.requestFailures.length ||
    summary.overflow.length ||
    summary.violations.length
  )
    process.exitCode = 1;
} finally {
  for (const context of contexts) await context.close();
  await browser.close();
  await new Promise((resolve) => server.close(resolve));
}
