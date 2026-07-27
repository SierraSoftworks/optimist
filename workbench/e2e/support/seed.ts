import { cpSync, mkdirSync, rmSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const designs = resolve(here, '../.designs')
const examples = resolve(here, '../../../examples')

/**
 * Gives each run a fresh copy of the shipped examples.
 *
 * The tests edit designs, and the server writes those edits to disk. Copying
 * rather than pointing at `examples/` keeps a test run from rewriting the worked
 * examples that documentation depends on.
 */
export default function seed() {
  rmSync(designs, { recursive: true, force: true })
  mkdirSync(designs, { recursive: true })
  for (const name of ['checkout', 'metastable']) {
    cpSync(resolve(examples, name), resolve(designs, name), { recursive: true })
  }
}
