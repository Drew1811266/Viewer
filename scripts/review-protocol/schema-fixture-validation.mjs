// Test-only evaluator for the exact schema vocabulary used by review fixtures, not a runtime reader.
import { readFile } from 'node:fs/promises'
import { isDeepStrictEqual } from 'node:util'

export async function validateFixture(value, definition, root = definition) {
  if (definition.$ref) {
    const [file, fragment = ''] = definition.$ref.split('#')
    const resolved = file ? JSON.parse(await readFile(`docs/protocol/${file}`, 'utf8')) : root
    const target = fragment.split('/').filter(Boolean).reduce((node, key) => node[key], resolved)
    return validateFixture(value, target, resolved)
  }
  if (definition.oneOf) {
    const results = await Promise.all(definition.oneOf.map(item => validateFixture(value, item, root)))
    if (results.filter(Boolean).length !== 1) return false
  }
  if (definition.anyOf && !(await Promise.all(definition.anyOf.map(item => validateFixture(value, item, root)))).some(Boolean)) return false
  if (Object.hasOwn(definition, 'const') && !isDeepStrictEqual(value, definition.const)) return false
  if (definition.enum && !definition.enum.some(item => isDeepStrictEqual(item, value))) return false
  if (definition.type) {
    const types = Array.isArray(definition.type) ? definition.type : [definition.type]
    const matches = type => type === 'null' ? value === null : type === 'array' ? Array.isArray(value)
      : type === 'object' ? value !== null && typeof value === 'object' && !Array.isArray(value)
      : type === 'integer' ? Number.isInteger(value) : typeof value === type
    if (!types.some(matches)) return false
  }
  if (typeof value === 'string') {
    const length = [...value].length
    if (length < (definition.minLength ?? 0) || length > (definition.maxLength ?? Infinity)) return false
    if (definition.pattern && !new RegExp(definition.pattern, 'u').test(value)) return false
  }
  if (typeof value === 'number' && (value < (definition.minimum ?? -Infinity) || value > (definition.maximum ?? Infinity))) return false
  if (typeof value === 'number' && definition.exclusiveMinimum !== undefined && value <= definition.exclusiveMinimum) return false
  if (Array.isArray(value)) {
    if (value.length < (definition.minItems ?? 0) || value.length > (definition.maxItems ?? Infinity)) return false
    if (definition.uniqueItems && value.some((item, index) => value.slice(0, index).some(previous => isDeepStrictEqual(item, previous)))) return false
    if (definition.items && !(await Promise.all(value.map(item => validateFixture(item, definition.items, root)))).every(Boolean)) return false
  }
  if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
    if (definition.required?.some(key => !Object.hasOwn(value, key))) return false
    if (definition.additionalProperties === false && Object.keys(value).some(key => !Object.hasOwn(definition.properties ?? {}, key))) return false
    for (const [key, item] of Object.entries(definition.properties ?? {})) {
      if (Object.hasOwn(value, key) && !await validateFixture(value[key], item, root)) return false
    }
  }
  return true
}
