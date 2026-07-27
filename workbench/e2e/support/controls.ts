import { type Page } from '@playwright/test'

/**
 * Clicks an Element Plus radio button by its label.
 *
 * The framework hides the real `<input type="radio">` and puts a styled span on
 * top of it, so targeting the radio role finds an element that is permanently
 * covered. Clicking the label is both what a person does and what actually
 * reaches the control.
 */
export async function chooseRadio(page: Page, label: string) {
  await page
    .locator('.el-radio-button__inner', { hasText: new RegExp(`^\\s*${escape(label)}\\s*$`) })
    .first()
    .click()
}

/**
 * Picks an option from a named select dropdown.
 *
 * Element Plus leaves every dropdown it has opened in the document and hides the
 * inactive ones, and closing is animated, so for a moment after opening a second
 * select both are on screen. Searching the page for an option by name can
 * therefore find one in a dropdown that is disappearing, and clicking it fails
 * halfway through. Each select is given its own `popper-class` so a test can say
 * which dropdown it means.
 *
 * An exact label match is preferred, because some options carry a description
 * under the name and a substring match would then be satisfied by the wrong one.
 */
export async function chooseOption(page: Page, popper: string, label: string) {
  const items = page.locator(`.${popper} .el-select-dropdown__item`)
  const exact = items.filter({ hasText: new RegExp(`^\\s*${escape(label)}\\s*$`) })
  const chosen = (await exact.count()) ? exact : items.filter({ hasText: label })
  await chosen.first().click()
}

function escape(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
