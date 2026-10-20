import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, type TestInfo } from "@playwright/test";

export async function signInAsDemo(page: Page) {
  await page.goto("/login");
  await page.getByRole("button", { name: "Fill in the demo account" }).click();
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page.waitForURL("**/today");
  await expect(page.getByRole("heading", { level: 1, name: "Today" })).toBeVisible();
}

/** Collects console errors and uncaught exceptions so a test can assert there were none. */
export function watchErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(err.message));
  return errors;
}

export async function shot(page: Page, info: TestInfo, name: string) {
  await page.screenshot({
    path: `e2e/screenshots/${info.project.name}-${name}.png`,
    fullPage: true,
  });
}

/** Fails on serious or critical WCAG A/AA violations. */
export async function expectAccessible(page: Page) {
  const result = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa"]).analyze();
  const blocking = result.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
  expect(
    blocking.map((v) => `${v.id}: ${v.help} (${v.nodes.map((n) => n.target.join(" ")).join(", ")})`),
  ).toEqual([]);
}

export async function expectNoHorizontalScroll(page: Page) {
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  expect(overflow).toBeLessThanOrEqual(0);
}
