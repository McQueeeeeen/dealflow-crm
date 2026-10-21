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
  await expect(page.getByRole("link", { name: "Ava Stone" })).toHaveCount(1);
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

test("the filter language narrows the board, explains mistakes and saves views", async ({
  page,
}, info) => {
  await page.goto("/pipeline");
  const filter = page.getByRole("searchbox", { name: "Filter deals" });
  await filter.fill("value>=150k valu:3");
  await expect(page.getByText(/unknown field “valu”/)).toBeVisible();

  await filter.fill("value>=150k status:open");
  await expect(page.getByText(/\d+ deals? match/)).toBeVisible();
  await expect(page).toHaveURL(/filter=value/);
  await expect(async () => {
    const amounts = await page.locator("article .num.font-medium").allTextContents();
    expect(amounts.length).toBeGreaterThan(0);
    for (const amount of amounts) {
      expect(Number(amount.replace(/[^0-9.]/g, ""))).toBeGreaterThanOrEqual(150_000);
    }
  }).toPass();

  const name = `Big open ${info.project.name} ${Date.now()}`;
  await page.getByRole("button", { name: "Save View" }).click();
  await page.getByLabel("Name").fill(name);
  await page.getByRole("button", { name: "Save View" }).click();
  const chip = page.getByRole("button", { name, exact: true });
  await expect(chip).toHaveAttribute("aria-pressed", "true");
  await chip.click();
  await expect(filter).toHaveValue("");
  page.once("dialog", (d) => d.accept());
  await page.getByRole("button", { name: `Delete view ${name}` }).click();
  await expect(chip).toHaveCount(0);
});

test("Reports simulate the quarter and the chance of reaching the target", async ({ page }) => {
  await page.goto("/reports");
  await expect(page.getByRole("heading", { name: "What Will Actually Close?" })).toBeVisible();
  await expect(page.getByText(/Most likely by/)).toBeVisible();
  await page.getByRole("button", { name: "This quarter" }).click();
  await expect(page.getByRole("button", { name: "This quarter" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(page.getByText(/chance to reach the .* target for this quarter/)).toBeVisible();
  await expect(page.getByRole("img", { name: /Distribution of simulated revenue/ })).toBeVisible();
});

test("an automation is written in plain terms and runs when deals change", async ({
  page,
}, info) => {
  await page.goto("/automations");
  await expect(page.getByRole("heading", { name: "Big proposals get a follow-up" })).toBeVisible();
  await expect(page.getByText("value>=100k")).toBeVisible();

  const name = `Note new deals ${info.project.name} ${Date.now()}`;
  await page.getByRole("button", { name: "New Automation" }).first().click();
  const dialog = page.getByRole("dialog", { name: "New Automation" });
  await dialog.getByLabel("Name").fill(name);
  await dialog.getByLabel("Only for deals matching (optional)").fill("valu>1");
  await dialog.getByLabel("Step 1").selectOption("add_note");
  await dialog.getByLabel("Note").fill("Created {deal} for {value}");
  await dialog.getByRole("button", { name: "Create Automation" }).click();
  await expect(dialog.getByText(/unknown field “valu”/)).toBeVisible();
  await dialog.getByLabel("Only for deals matching (optional)").fill("value>1");
  await dialog.getByRole("button", { name: "Create Automation" }).click();
  await expect(dialog).toBeHidden();

  const card = page.getByRole("region", { name: "Rules" }).locator("section", { hasText: name });
  await expect(card.getByText("When a deal is created")).toBeVisible();
  await expect(card.getByText("Add the note “Created {deal} for {value}”")).toBeVisible();

  const title = `Automated ${info.project.name} ${Date.now()}`;
  const created = await page.request.post("/api/v1/deals", {
    data: { title, value_cents: 250_000 },
  });
  expect(created.status()).toBe(201);
  await page.reload();
  await expect(page.getByRole("link", { name: title }).first()).toBeVisible();

  page.once("dialog", (d) => d.accept());
  await card.getByRole("button", { name: "Delete" }).click();
  await expect(card).toHaveCount(0);
});

test("duplicate contacts are found and merged with a choice per field", async ({ page }, info) => {
  const tag = `${info.project.name}${Date.now()}`;
  const email = `twin.${tag}@example.com`;
  for (const [first, last, phone] of [
    ["Nora", `Twin${tag}`, null],
    ["Nora", `Twinn${tag}`, "+1 555 0199"],
  ]) {
    const res = await page.request.post("/api/v1/contacts", {
      data: { first_name: first, last_name: last, email, phone },
    });
    expect(res.status()).toBe(201);
  }

  await page.goto("/contacts");
  await page.getByRole("link", { name: "Find Duplicates" }).click();
  await expect(page.getByRole("heading", { name: "Possible Duplicate Contacts" })).toBeVisible();
  const pair = page.locator("section", { hasText: `Twinn${tag}` });
  await expect(pair.getByText("Same email")).toBeVisible();
  await pair.getByRole("radio", { name: `Twinn${tag}` }).check();
  await pair.getByRole("button", { name: /^Merge into/ }).click();
  await expect(pair).toHaveCount(0);

  await page.goto("/contacts");
  await page.getByRole("searchbox", { name: "Search contacts" }).fill(email);
  await expect(page.getByRole("link", { name: `Nora Twinn${tag}` })).toHaveCount(1);
});

test("an edit is undone from the deal's history", async ({ page }, info) => {
  await page.goto("/pipeline");
  await page.locator("article a").first().click();
  await page.waitForURL(/\/deals\//);
  const heading = page.getByRole("heading", { level: 1 });
  const original = (await heading.textContent())!.trim();

  await page.getByRole("button", { name: "Edit Deal" }).click();
  const dialog = page.getByRole("dialog", { name: "Edit Deal" });
  const renamed = `${original} (${info.project.name} draft)`;
  await dialog.getByLabel("Deal name").fill(renamed);
  await dialog.getByRole("button", { name: "Save Deal" }).click();
  await expect(heading).toHaveText(renamed);

  await page.getByRole("button", { name: "Undo" }).first().click();
  await expect(heading).toHaveText(original);
});
