// The site's design tokens are a copy of the monorepo template's web App
// tokens (docs/specs/site-design-system.md, decision 1). This test fails,
// naming the block, when the copy and the template differ, so a palette
// change to the template reaches the site in the same pull request.
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

const TEMPLATE_CSS = new URL(
  '../../crates/rok-templates/templates/monorepo/apps/web/src/styles.css',
  import.meta.url,
)
const SITE_CSS = new URL('./styles.css', import.meta.url)

/** The blocks copied verbatim, by the line that opens each. */
const BLOCKS = [':root {', '.dark {', '@theme inline {', '@layer base {']

function read(url: URL): string[] {
  return readFileSync(fileURLToPath(url), 'utf8')
    .replace(/\r\n/g, '\n')
    .split('\n')
}

/**
 * The block opened by `header` (a line at column 0) up to its closing `}`
 * at column 0. Throws when the header is missing or appears more than once,
 * since either makes "the copy" ambiguous.
 */
function block(lines: string[], header: string, file: string): string {
  const starts = lines.flatMap((line, i) => (line === header ? [i] : []))
  if (starts.length !== 1) {
    throw new Error(
      `${file}: expected one \`${header}\` block, found ${starts.length}`,
    )
  }
  const end = lines.indexOf('}', starts[0])
  if (end < 0) throw new Error(`${file}: \`${header}\` block is not closed`)
  return lines.slice(starts[0], end + 1).join('\n')
}

describe('design tokens copied from the web template', () => {
  const template = read(TEMPLATE_CSS)
  const site = read(SITE_CSS)

  it.each(BLOCKS)('`%s` matches the template', (header) => {
    const expected = block(template, header, 'template styles.css')
    const actual = block(site, header, 'site styles.css')
    expect(
      actual,
      `site/src/styles.css: the \`${header}\` block differs from ` +
        'crates/rok-templates/templates/monorepo/apps/web/src/styles.css; ' +
        'copy the template block verbatim',
    ).toBe(expected)
  })
})
