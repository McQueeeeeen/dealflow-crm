import { expect, test } from "@playwright/test";

import {
  expectAccessible,
  expectNoHorizontalScroll,
  shot,
  signInAsDemo,
  watchErrors,
} from "./helpers";

for (const scheme of ["light", "dark"] as const) {
  test(`every screen renders cleanly in ${scheme} mode`, async ({ page }, info) => {
    test.setTimeout(90_000);
    await page.addInitScript((t) => localStorage.setItem("dealflow-theme", t), scheme);
    const errors = watchErrors(page);

    await page.goto("/login");
    await expectAccessible(page);
    await shot(page, info, `${scheme}-login`);
    await signInAsDemo(page);

    const screens: { path: string; heading: RegExp | string; name: string }[] = [
      { path: "/today", heading: /^Welcome back/, name: "today" },
      { path: "/pipeline", heading: "Pipeline", name: "pipeline" },
      { path: "/companies", heading: "Companies", name: "companies" },
      { path: "/contacts", heading: "Contacts", name: "contacts" },
      { path: "/reports", heading: "Reports", name: "reports" },
      { path: "/settings", heading: "Settings", name: "settings" },
    ];
    for (const screen of screens) {
      await page.goto(screen.path);
      await expect(page.getByRole("heading", { level: 1, name: screen.heading })).toBeVisible();
      await page.waitForLoadState("networkidle");
      await expectAccessible(page);
      await expectNoHorizontalScroll(page);
      await shot(page, info, `${scheme}-${screen.name}`);
    }

    await page.goto("/today");
    await expect(page.getByRole("heading", { level: 1, name: /^Welcome back/ })).toBeVisible();
    await page.keyboard.press("ControlOrMeta+k");
    await page.getByRole("combobox").fill("fleet");
    await expect(page.getByRole("option").first()).toBeVisible();
    await expectAccessible(page);
    await shot(page, info, `${scheme}-palette`);
    await page.keyboard.press("Escape");

    await page.goto("/companies");
    await page.locator("table a").first().click();
    await page.waitForURL(/\/companies\/[0-9a-f-]+$/);
    await page.waitForLoadState("networkidle");
    await expectAccessible(page);
    await shot(page, info, `${scheme}-company`);

    await page.goto("/contacts");
    await page.locator("table a").first().click();
    await page.waitForURL(/\/contacts\/[0-9a-f-]+$/);
    await page.waitForLoadState("networkidle");
    await expectAccessible(page);
    await shot(page, info, `${scheme}-contact`);

    await page.goto("/pipeline");
    await page.locator("article a").first().click();
    await page.waitForURL(/\/deals\/[0-9a-f-]+$/);
    await page.waitForLoadState("networkidle");
    await expectAccessible(page);
    await expectNoHorizontalScroll(page);
    await shot(page, info, `${scheme}-deal`);

    expect(errors).toEqual([]);
  });
}
