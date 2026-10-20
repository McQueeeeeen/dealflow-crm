import { expect, test } from "@playwright/test";

import { signInAsDemo, watchErrors } from "./helpers";

test.beforeEach(async ({ page }) => {
  await signInAsDemo(page);
});

test("a deal moves to the next stage with the keyboard", async ({ page }, info) => {
  test.skip(info.project.name === "mobile", "keyboard dragging is a desktop interaction");
  const errors = watchErrors(page);
  await page.goto("/pipeline");

  const lead = page.getByRole("region", { name: "Lead" });
  const qualified = page.getByRole("region", { name: "Qualified" });
  const handle = lead.getByRole("button", { name: /^Move / }).first();
  const title = (await handle.getAttribute("aria-label"))!.replace(/^Move /, "");

  await handle.focus();
  const moved = page.waitForResponse(
    (res) => res.url().includes("/move") && res.request().method() === "POST",
  );
  const announcer = page.locator("[aria-live]").filter({ hasText: title });
  await page.keyboard.press("Space");
  await expect(announcer).toContainText(`Picked up ${title}`);
  await page.keyboard.press("ArrowRight");
  await expect(announcer).toContainText("is over Qualified");
  await page.keyboard.press("Space");
  expect((await moved).status()).toBe(200);

  await expect(qualified.getByRole("link", { name: title, exact: true })).toBeVisible();
  await expect(lead.getByRole("link", { name: title, exact: true })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("a follow-up is created and completed from Today", async ({ page }, info) => {
  const title = `Call back about pricing (${info.project.name})`;
  await page.getByRole("button", { name: "Add follow-up" }).first().click();
  const dialog = page.getByRole("dialog", { name: "New follow-up" });
  await dialog.getByLabel("What needs to happen").fill(title);
  await dialog.getByRole("button", { name: "Add follow-up" }).click();
  await expect(dialog).toBeHidden();

  const done = page.getByRole("checkbox", { name: `Mark "${title}" done` });
  await expect(done).toBeVisible();
  await done.check();
  await expect(page.getByText(title)).toHaveCount(0);
});

test("validation errors are shown next to the field", async ({ page }) => {
  await page.goto("/pipeline");
  await page.getByRole("button", { name: "New deal" }).click();
  const dialog = page.getByRole("dialog", { name: "New deal" });
  await dialog.getByRole("button", { name: "Create deal" }).click();
  await expect(dialog.getByText("is required")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

test("a note is added to a deal and appears in its history", async ({ page }, info) => {
  await page.goto("/pipeline");
  await page.locator("article a").first().click();
  await page.waitForURL(/\/deals\//);

  const body = `Budget confirmed on the call (${info.project.name}).`;
  await page.getByLabel("New note").fill(body);
  await page.getByRole("button", { name: "Save note" }).click();
  await expect(page.getByText(body)).toBeVisible();
});

test("the theme toggle switches and remembers the theme", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.reload();
  await page.getByRole("button", { name: "Switch to dark theme" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("button", { name: "Switch to light theme" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
});

test("signing out ends the session", async ({ page }) => {
  await page.getByRole("button", { name: "Account" }).click();
  await page.getByRole("button", { name: "Sign out" }).click();
  await page.waitForURL("**/login");
  await page.goto("/today");
  await page.waitForURL(/\/login/);
});
