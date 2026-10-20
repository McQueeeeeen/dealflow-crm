import { expect, test } from "@playwright/test";

import { signInAsDemo, watchErrors } from "./helpers";

test.beforeEach(async ({ page }) => {
  await signInAsDemo(page);
});

test("the command palette finds a company despite a typo", async ({ page }) => {
  const errors = watchErrors(page);
  await page.keyboard.press("ControlOrMeta+k");
  const input = page.getByRole("combobox", { name: "Search deals, companies and people" });
  await expect(input).toBeFocused();
  await input.fill("copperleef foods");
  const option = page.getByRole("option", { name: /Copperleaf Foods/ });
  await expect(option).toBeVisible();
  await page.keyboard.press("Enter");
  await page.waitForURL(/\/companies\/[0-9a-f-]+$/);
  await expect(page.getByRole("heading", { level: 1, name: "Copperleaf Foods" })).toBeVisible();
  expect(errors).toEqual([]);
});

test("a teammate's new deal appears on the board without a reload", async ({
  page,
  request,
}, info) => {
  await page.goto("/pipeline");
  await expect(page.getByRole("status").filter({ hasText: /^Live$/ })).toBeVisible({
    timeout: 15_000,
  });

  const login = await request.post("/api/auth/login", {
    data: { email: "priya@dealflow.dev", password: "dealflow-demo" },
  });
  expect(login.ok()).toBeTruthy();
  const title = `Live renewal (${info.project.name} ${Date.now()})`;
  const created = await request.post("/api/v1/deals", { data: { title, value_cents: 990_000 } });
  expect(created.status()).toBe(201);

  await expect(page.getByRole("link", { name: title, exact: true })).toBeVisible({
    timeout: 15_000,
  });
});

test("contacts import from CSV after a checked preview", async ({ page }, info) => {
  await page.goto("/contacts");
  await page.getByRole("button", { name: "Import CSV" }).click();
  const dialog = page.getByRole("dialog", { name: "Import contacts" });
  const email = `ava.${info.project.name}.${Date.now()}@example.com`;
  await dialog.getByLabel("CSV file").setInputFiles({
    name: "contacts.csv",
    mimeType: "text/csv",
    buffer: Buffer.from(
      `First Name,Last Name,E-mail,Company\r\nAva,Stone,${email},Stone Labs\r\n,Missing,nobody@example.com,\r\n`,
    ),
  });
  await dialog.getByRole("button", { name: "Check Rows" }).click();
  await expect(dialog.getByText("Nothing is saved yet")).toBeVisible();
  await expect(dialog.getByText("first name is required")).toBeVisible();
  await dialog.getByRole("button", { name: "Import 1 Rows" }).click();
  await expect(dialog.getByText("Import done.")).toBeVisible();
  await dialog.getByRole("button", { name: "Done" }).click();

  await page.getByRole("searchbox", { name: "Search contacts" }).fill(email);
  await expect(page.getByRole("link", { name: "Ava Stone" })).toBeVisible();
});

test("lists export as CSV files", async ({ page }) => {
  await page.goto("/companies");
  const download = page.waitForEvent("download");
  await page.getByRole("link", { name: "Export CSV" }).click();
  expect((await download).suggestedFilename()).toMatch(
    /^dealflow-companies-\d{4}-\d{2}-\d{2}\.csv$/,
  );
});

test("Today flags quiet deals and Reports tracks targets", async ({ page }) => {
  await expect(page.getByRole("heading", { name: "Going Cold" })).toBeVisible();
  await page.goto("/reports");
  await expect(page.getByRole("heading", { name: "Monthly Targets" })).toBeVisible();
  await expect(page.getByRole("img", { name: /won of/ }).first()).toBeVisible();
});
