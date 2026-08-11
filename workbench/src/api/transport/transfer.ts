import { ApiError } from '../errors'
import type { Imported } from './contract'

/**
 * A directory name, matching the rule the server enforces.
 *
 * The file a person chose is what the design gets called, because they already
 * named it once and asking again would be asking the same question twice.
 */
export function designNamed(file: string): string {
  const id = file
    .replace(/\.zip$/i, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 128)
  if (id) return id
  throw new ApiError(400, `'${file}' cannot name a design.`, [
    'Rename the file using letters and digits, then choose it again.',
  ])
}

/**
 * Stores an archive, turning a refusal to overwrite into a question.
 *
 * Replacing a design loses whatever it held, so it is something a person says
 * rather than something a file name decides on their behalf. The retry is
 * handed back as a function so that each host keeps whatever it needs to send
 * the same archive again — a blob here, a path there — without saying what.
 */
export function stored(
  design: string,
  put: (replace: boolean) => Promise<unknown>,
): Promise<Imported> {
  const attempt = async (replace: boolean): Promise<Imported> => {
    try {
      await put(replace)
      return { status: 'stored', design }
    } catch (error) {
      if (error instanceof ApiError && error.status === 409) {
        return { status: 'conflict', design, replace: () => attempt(true) }
      }
      throw error
    }
  }
  return attempt(false)
}
