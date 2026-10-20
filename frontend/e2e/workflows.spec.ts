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
  await page.keyboard.press("ArrowRight");
  expect((await moved).status()).toBe(200);

  await expect(qualified.getByRole("link", { name: title, exact: true })).toBeVisible();
  await expect(lead.getByRole("link", { name: title, exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: `Move ${title}` })).toBeFocused();
  await expect(
    page.getByRole("status").filter({ hasText: `${title} moved to Qualified.` }),
  ).toBeAttached();
  expect(errors).toEqual([]);
});

test("a follow-up is created and completed from Today", async ({ page }, info) => {
  const title = `Call back about pricing (${info.project.name})`;
  await page.getByRole("button", { name: "Add Follow-Up" }).first().click();
  const dialog = page.getByRole("dialog", { name: "New Follow-Up" });
  await dialog.getByLabel("What needs to happen").fill(title);
  await dialog.getByRole("button", { name: "Add Follow-Up" }).click();
  await expect(dialog).toBeHidden();

  const done = page.getByRole("checkbox", { name: `Mark “${title}” done` });
  await expect(done).toBeVisible();
  await done.check();
  await expect(page.getByText(title)).toHaveCount(0);
});

test("validation errors are shown next to the field", async ({ page }) => {
  await page.goto("/pipeline");
  await page.getByRole("button", { name: "New Deal" }).click();
  const dialog = page.getByRole("dialog", { name: "New Deal" });
  await dialog.getByRole("button", { name: "Create Deal" }).click();
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
  await page.getByRole("button", { name: "Save Note" }).click();
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
  await page.getByRole("button", { name: "Sign Out" }).click();
  await page.waitForURL("**/login");
  await page.goto("/today");
  await page.waitForURL(/\/login/);
});

test("the stage rail moves a deal from its page", async ({ page }) => {
  await page.goto("/pipeline");
  await page.getByRole("region", { name: "Lead" }).locator("article a").first().click();
  await page.waitForURL(/\/deals\//);
  const rail = page.getByRole("region", { name: "Stage" });
  await rail.getByRole("button", { name: "Move to Proposal" }).click();
  await expect(rail.locator('[aria-current="step"]')).toContainText("Proposal");
  await expect(page.getByText("50% likely to close.")).toBeVisible();
});
