import bbobHTML from '@bbob/html'
import presetHTML5 from '@bbob/preset-html5'
import DOMPurify from 'dompurify'
import { isEOL } from '@bbob/plugin-helper'

// `presetHTML5` renders bold/italic/underline/strike/color/size as inline `style` attributes, and
// DOMPurify does not inspect the content of a `style` attribute, so an attacker could otherwise
// inject arbitrary CSS (`position: fixed` overlays, `url()` beacons, ...) into any BBCode field
const ALLOWED_CSS_PROPERTIES = ['color', 'font-weight', 'font-style', 'font-size', 'text-decoration']

DOMPurify.addHook('uponSanitizeAttribute', (_node, data) => {
  if (data.attrName !== 'style') return

  data.attrValue = data.attrValue
    .split(';')
    .map((declaration) => declaration.trim())
    .filter((declaration) => {
      const property = declaration.split(':')[0]?.trim().toLowerCase()
      return property !== undefined && property !== '' && ALLOWED_CSS_PROPERTIES.includes(property)
    })
    .join('; ')
})

export function parseBBCode(bbcode: string): string {
  if (!bbcode) return ''

  const html = bbobHTML(bbcode, [presetHTML5(), lineBreakPlugin()])

  // sanitize html to prevent xss attacks
  return DOMPurify.sanitize(html)
}

//TODO: properly type this function
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const walk = (t: any) => {
  const tree = t

  if (Array.isArray(tree)) {
    for (let idx = 0; idx < tree.length; idx++) {
      const child = walk(tree[idx])
      if (Array.isArray(child)) {
        tree.splice(idx, 1, ...child)
        idx += child.length - 1
      } else {
        tree[idx] = child
      }
    }
  } else if (tree && typeof tree === 'object' && tree.content) {
    if (tree.disableLineBreakConversion) {
      // stop walk. children won't be parsed to have <br>
      return tree.tag ? tree : tree.content
    }
    walk(tree.content)
  }

  if (isEOL(tree)) {
    return [{ tag: 'br', content: null }, '\n']
  }

  return tree
}

export const lineBreakPlugin = () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (tree: any) => walk(tree)
}
