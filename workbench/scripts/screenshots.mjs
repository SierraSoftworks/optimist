import { mkdirSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { chromium, selectors } from '@playwright/test'

/**
 * Captures the workbench screenshots the documentation embeds.
 *
 * Run it against a server holding the shipped examples, so what the guides show
 * is the design they describe rather than a mock-up that drifts from it:
 *
 *   optimist serve --designs ./examples --bind 127.0.0.1:3211
 *   OPTIMIST_API_URL=http://127.0.0.1:3211 npm run dev -- --port 5175
 *   npm run screenshots
 */
const here = dirname(fileURLToPath(import.meta.url))
const out = resolve(here, '../../docs/.vuepress/public/screenshots')
const base = process.env.WORKBENCH_URL ?? 'http://127.0.0.1:5175'

mkdirSync(out, { recursive: true })

// The markup uses `data-test`; the default would look for `data-testid`.
selectors.setTestIdAttribute('data-test')

const browser = await chromium.launch()
const page = await browser.newPage({
  viewport: { width: 1440, height: 860 },
  deviceScaleFactor: 2,
})

async function shoot(name, clip) {
  await page.screenshot({ path: resolve(out, `${name}.png`), clip })
  console.log(`captured ${name}.png`)
}

/**
 * Stops on the graph node under the flyout it opens.
 *
 * Cytoscape draws to a canvas, so there is no element to hover. The diagram is
 * laid out along the direction demand travels, which puts the components down
 * the middle of the frame; sweeping that line finds one without the script
 * needing to know where the layout put it.
 */
async function hoverGraphNode(fraction) {
  const box = await page.locator('.graph').boundingBox()
  for (let step = 0; step < 40; step += 1) {
    await page.mouse.move(
      box.x + box.width * fraction.x,
      box.y + box.height * (0.1 + step * 0.02),
    )
    if (await page.getByTestId('component-limits').isVisible()) return
  }
  throw new Error('no component was found under the sweep')
}

await page.goto(`${base}/`)
await page.getByTestId('open-metastable').waitFor()
// The picker is a short list on a tall page, so only the part with anything in it.
await shoot('designs', { x: 0, y: 0, width: 1440, height: 480 })

await page.goto(`${base}/d/metastable/design/checkout`)
await page.getByTestId('solve-ok').waitFor({ timeout: 60_000 })
await page.getByTestId('component-name').waitFor()
await shoot('design')

await page.goto(`${base}/d/metastable/design`)
await page.getByTestId('solve-ok').waitFor({ timeout: 60_000 })
await hoverGraphNode({ x: 0.45 })
await shoot('limits')

await page.goto(`${base}/d/metastable/design`)
await page.getByTestId('solve-ok').waitFor({ timeout: 60_000 })
await page.getByTestId('quantity-expression-store_service_time').locator('.cm-content').click()
await page.getByTestId('quantity-preview').locator('svg').waitFor({ timeout: 30_000 })
await shoot('quantities')

await page.goto(`${base}/d/metastable/review`)
await page.locator('figure svg.plot').first().waitFor({ timeout: 60_000 })
await page.getByTestId('limit-cards').waitFor()
await shoot('simulation')

await page.goto(`${base}/d/metastable/review/shed`)
await page.locator('polyline.reference').first().waitFor({ timeout: 60_000 })
await shoot('comparison')

await browser.close()
