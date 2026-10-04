import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { checkBrowser } from "./browser-app.mjs";

const root = resolve(import.meta.dirname, "..");
const navigation = JSON.parse(await readFile(resolve(root, "apps/docs/navigation.json"), "utf8"));

await checkBrowser("apps", async (page, origin) => {
  await page.setViewportSize({ width: 1440, height: 1100 });
  await page.clock.install({ time: "2026-01-01T00:00:00Z" });
  await page.goto(origin);
  await page.getByRole("heading", { name: "Ultrafast SQL Client in your Terminal" }).waitFor();
  const installer = await page.request.get(origin + "install.sh");
  assert.equal(installer.status(), 200);
  assert.equal(await installer.text(), await readFile(resolve(root, "install.sh"), "utf8"));
  await page.locator(".example").hover();
  await checkWalkthrough(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  const methods = page.getByRole("group", { name: "Installation method" });
  assert.equal(await methods.getByRole("button", { name: "Cargo" }).getAttribute("aria-pressed"),
    "true");
  for (const [name, command] of [
    ["Cargo", "cargo install sql-bomb --locked"],
    ["Linux", "curl -fsSL https://boom.fusor.build/install.sh | sh"],
  ]) {
    const button = methods.getByRole("button", { name, exact: true });
    await button.click();
    assert.equal(await button.getAttribute("aria-pressed"), "true");
    assert.equal(await page.locator("#install-command code").textContent(), command);
    await page.getByRole("button", { name: "Copy command", exact: true }).click();
    await page.getByRole("button", { name: "Copied!", exact: true }).waitFor();
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), command);
  }
  await methods.getByRole("button", { name: "Cargo" }).click();
  await page.screenshot({ path: resolve(root, "apps/target/landing-desktop.png"), fullPage: true });
  for (const width of [320, 390, 768]) {
    await page.setViewportSize({ width, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth), width);
  }
  await page.emulateMedia({ reducedMotion: "reduce" });
  assert.equal(await page.locator(".example").evaluate(node =>
    getComputedStyle(node).animationName), "none");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: resolve(root, "apps/target/landing-mobile.png"), fullPage: true });
  await page.getByRole("link", { name: "Documentation", exact: false }).click();
  assert.equal(new URL(page.url()).pathname, "/docs/");
  await checkDocs(page, origin);
}, ["--site"]);
console.log("PASS: native walkthrough, clipboard, responsive layout, docs, routing and themes");

async function checkWalkthrough(page) {
  const controls = page.getByRole("group", { name: "Explore sql-bomb", exact: true });
  const capture = page.locator(".terminal-capture");
  for (const [name, filename, expected] of [
    ["Workspace", "welcome", "Ready"],
    ["Query", "results", "Montréal"],
    ["Inspect", "inspector", "Utf8"],
    ["Library", "library", "SQL PREVIEW"],
    ["Menu", "menu", "Find an action"],
  ]) {
    const button = controls.getByRole("button", { name, exact: true });
    await button.focus();
    await page.keyboard.press("Enter");
    assert.equal(await button.getAttribute("aria-pressed"), "true");
    assert.equal(await controls.locator('[aria-pressed="true"]').count(), 1);
    assert.equal(await capture.getAttribute("src"), `/terminal/${filename}.svg`);
    await capture.evaluate(image => image.decode());
    const response = await page.request.get(new URL(await capture.getAttribute("src"), page.url()).href);
    assert.equal(response.status(), 200);
    assert.ok((await response.text()).includes(expected));
  }
  await controls.getByRole("button", { name: "Query", exact: true }).click();
  const outside = page.getByRole("heading", { name: "Ultrafast SQL Client in your Terminal" });
  const clockStep = "00:03";
  await page.clock.pauseAt("2026-01-01T00:01:00Z");
  await page.clock.fastForward(clockStep);
  assert.equal(await capture.getAttribute("src"), "/terminal/results.svg");
  await outside.hover();
  for (const filename of ["inspector", "library", "menu", "welcome", "results"]) {
    await page.clock.fastForward(clockStep);
    assert.equal(await capture.getAttribute("src"), `/terminal/${filename}.svg`);
  }
  await capture.hover();
  await page.clock.fastForward(clockStep);
  assert.equal(await capture.getAttribute("src"), "/terminal/results.svg");
  await controls.getByRole("button", { name: "Menu", exact: true }).click();
  await page.clock.fastForward(clockStep);
  assert.equal(await capture.getAttribute("src"), "/terminal/menu.svg");
  await outside.hover();
  await page.clock.fastForward(clockStep);
  assert.equal(await capture.getAttribute("src"), "/terminal/welcome.svg");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.clock.fastForward(clockStep);
  assert.equal(await capture.getAttribute("src"), "/terminal/welcome.svg");
  await controls.getByRole("button", { name: "Query", exact: true }).click();
  assert.equal(await capture.getAttribute("src"), "/terminal/results.svg");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.clock.resume();
}

async function checkDocs(page, origin) {
  for (const { slug } of navigation) {
    const filename = `${slug || "index"}.md`;
    const markdown = await readFile(resolve(root, "docs", filename), "utf8");
    const title = markdown.split("\n")[0].slice(2);
    await page.goto(origin + "docs/" + slug);
    await page.getByRole("heading", { name: title, exact: true }).waitFor();
    assert.equal(await page.title(), `${title} · sql-bomb`);
    await page.reload();
    await page.getByRole("heading", { name: title, exact: true }).waitFor();
    const bodySizes = await page.locator(".article .lead, .article .markdown").evaluateAll(nodes =>
      [...new Set(nodes.map(node => getComputedStyle(node).fontSize))]);
    assert.deepEqual(bodySizes, ["16px"]);
    const response = await page.request.get(origin + "docs/content/" + filename);
    assert.equal(response.status(), 200);
    assert.equal(await response.text(), markdown);
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth), 390);
  }
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(origin + "docs/");
  await page.getByRole("searchbox").fill("no-matching-topic");
  await page.locator(".search-empty").waitFor();
  await page.getByRole("searchbox").press("Escape");
  await page.getByRole("button", { name: "Toggle color theme" }).click();
  await page.reload();
  await page.locator(".site.dark").waitFor();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Toggle navigation" }).click();
  await page.locator(".sidebar").getByRole("link", { name: "Get started", exact: true }).click();
  await page.getByRole("heading", { name: "Get started", exact: true }).waitFor();
  await page.locator(".sidebar").waitFor({ state: "hidden" });
  await page.screenshot({ path: resolve(root, "apps/target/docs-mobile.png"), fullPage: true });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.screenshot({ path: resolve(root, "apps/target/docs-desktop.png"), fullPage: true });
  await page.goto(origin + "docs/missing");
  await page.getByRole("heading", { name: "Page not found", exact: true }).waitFor();
  await page.getByRole("link", { name: "Home", exact: true }).click();
  await page.getByRole("heading", { name: "Ultrafast SQL Client in your Terminal" }).waitFor();
}
