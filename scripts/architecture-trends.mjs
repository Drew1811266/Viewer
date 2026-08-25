#!/usr/bin/env node

import {
  existsSync,
  readFileSync,
  readdirSync,
} from 'node:fs'
import {
  extname,
  join,
  posix,
  relative,
  resolve,
} from 'node:path'
import { pathToFileURL } from 'node:url'

const SCHEMA_VERSION = 1
const normalizePath = (path) => path.replaceAll('\\', '/')
const countLines = (source) => source.split(/\r?\n/).length - 1

const isWholeTestFile = (path) => {
  const normalized = normalizePath(path)
  return /\.test\.tsx?$/.test(normalized) || /(?:^|\/)tests\//.test(normalized)
}

const maskNonCode = (source, language) => {
  const output = [...source]
  const blank = (start, end) => {
    for (let index = start; index < end; index += 1) {
      if (output[index] !== '\n' && output[index] !== '\r') output[index] = ' '
    }
  }
  const consumeQuoted = (start, quote) => {
    let index = start + 1
    while (index < source.length) {
      if (source[index] === '\\') {
        index += 2
        continue
      }
      index += 1
      if (source[index - 1] === quote) break
    }
    blank(start, index)
    return index
  }

  let index = 0
  while (index < source.length) {
    if (source.startsWith('//', index)) {
      const end = source.indexOf('\n', index + 2)
      const next = end === -1 ? source.length : end
      blank(index, next)
      index = next
      continue
    }
    if (source.startsWith('/*', index)) {
      const end = source.indexOf('*/', index + 2)
      const next = end === -1 ? source.length : end + 2
      blank(index, next)
      index = next
      continue
    }
    if (language === 'rust') {
      const raw = source.slice(index).match(/^(?:br|r)(#{0,16})"/)
      if (raw) {
        const delimiter = `"${raw[1]}`
        const end = source.indexOf(delimiter, index + raw[0].length)
        const next = end === -1 ? source.length : end + delimiter.length
        blank(index, next)
        index = next
        continue
      }
    }
    if (source[index] === '"' || (
      language === 'typescript' && (source[index] === "'" || source[index] === '`')
    )) {
      index = consumeQuoted(index, source[index])
      continue
    }
    if (
      language === 'rust'
      && source[index] === "'"
      && /^'(?:\\.|[^\\'\r\n])'/.test(source.slice(index))
    ) {
      index = consumeQuoted(index, "'")
      continue
    }
    index += 1
  }

  return output.join('')
}

const lineIndexAtOffset = (source, offset) => {
  let line = 0
  for (let index = 0; index < offset; index += 1) {
    if (source[index] === '\n') line += 1
  }
  return line
}

const skipWhitespace = (source, start) => {
  let index = start
  while (/\s/.test(source[index] ?? '')) index += 1
  return index
}

const skipAttribute = (source, start) => {
  let index = start
  if (source[index] !== '#') return undefined
  index += 1
  if (source[index] === '!') index += 1
  index = skipWhitespace(source, index)
  if (source[index] !== '[') return undefined

  let depth = 0
  for (; index < source.length; index += 1) {
    if (source[index] === '[') depth += 1
    if (source[index] === ']') {
      depth -= 1
      if (depth === 0) return index + 1
    }
  }
  return undefined
}

const rustTestModules = (source) => {
  const totalLines = countLines(source)
  const masked = maskNonCode(source, 'rust')
  const modules = []
  const cfgTest = /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/g

  for (const attribute of masked.matchAll(cfgTest)) {
    let index = attribute.index + attribute[0].length
    while (true) {
      index = skipWhitespace(masked, index)
      const afterAttribute = skipAttribute(masked, index)
      if (afterAttribute === undefined) break
      index = afterAttribute
    }
    index = skipWhitespace(masked, index)

    const module = masked.slice(index).match(
      /^(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([A-Za-z_]\w*)\b/,
    )
    if (!module) continue
    index += module[0].length
    index = skipWhitespace(masked, index)

    const startLine = lineIndexAtOffset(masked, attribute.index)
    if (masked[index] === ';') {
      modules.push({
        name: module[1],
        startLine,
        endLine: lineIndexAtOffset(masked, index),
        external: true,
      })
      continue
    }
    if (masked[index] !== '{') continue

    let depth = 0
    let close = -1
    for (let brace = index; brace < masked.length; brace += 1) {
      if (masked[brace] === '{') depth += 1
      if (masked[brace] === '}') {
        depth -= 1
        if (depth === 0) {
          close = brace
          break
        }
      }
    }
    if (close === -1) continue
    modules.push({
      name: module[1],
      startLine,
      endLine: Math.min(lineIndexAtOffset(masked, close), totalLines - 1),
      external: false,
    })
  }

  return modules
}

const allLineIndexes = (source) =>
  new Set(Array.from({ length: countLines(source) }, (_, index) => index))

const classifyTestLines = (files) => {
  const sources = new Map([...files]
    .map(([path, source]) => [normalizePath(path), source])
    .sort(([left], [right]) => left.localeCompare(right)))
  const classified = new Map([...sources].map(([path]) => [path, new Set()]))
  const externalModules = []

  for (const [path, source] of sources) {
    if (isWholeTestFile(path)) {
      classified.set(path, allLineIndexes(source))
      continue
    }
    if (!path.endsWith('.rs')) continue

    for (const module of rustTestModules(source)) {
      const lines = classified.get(path)
      for (let index = module.startLine; index <= module.endLine; index += 1) {
        lines.add(index)
      }
      if (module.external) externalModules.push({ path, name: module.name })
    }
  }

  for (const module of externalModules) {
    const directory = posix.dirname(module.path)
    const candidates = [
      posix.join(directory, `${module.name}.rs`),
      posix.join(directory, module.name, 'mod.rs'),
    ]
    for (const candidate of candidates) {
      const source = sources.get(candidate)
      if (source !== undefined) classified.set(candidate, allLineIndexes(source))
    }
  }

  return { sources, classified }
}

export function measureSourceFiles(files) {
  const { sources, classified } = classifyTestLines(files)
  const measured = [...sources]
    .map(([path, source]) => {
      const lines = countLines(source)
      const testLines = classified.get(path).size
      return {
        path,
        lines: lines - testLines,
        testLines,
      }
    })
    .sort((left, right) => left.path.localeCompare(right.path))

  const productionLines = measured.reduce((sum, file) => sum + file.lines, 0)
  const testLines = measured.reduce((sum, file) => sum + file.testLines, 0)
  return {
    productionLines,
    testLines,
    testToProductionRatio: productionLines === 0 ? 0 : testLines / productionLines,
    filesOver1000Lines: measured
      .filter((file) => file.lines > 1000)
      .map(({ path, lines }) => ({ path, lines })),
  }
}

const declarationFromLine = (path, line) => {
  if (path.endsWith('.rs')) {
    const declaration = line.match(
      /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+"[^"]+"\s+)?fn\s+([A-Za-z_]\w*)\b/,
    )
    if (!declaration) return undefined
    return {
      name: declaration[1],
      kind: 'rust',
      headEndColumn: declaration[0].length,
    }
  }
  if (!/\.tsx?$/.test(path)) return undefined

  const functionDeclaration = line.match(
    /^\s*(?:export\s+(?:default\s+)?)?(?:async\s+)?function\s+([A-Za-z_$][\w$]*)\b/,
  )
  if (functionDeclaration) {
    return {
      name: functionDeclaration[1],
      kind: 'typescript-function',
      headEndColumn: functionDeclaration[0].length,
    }
  }
  const hookDeclaration = line.match(
    /^\s*(?:export\s+)?const\s+(use[A-Z0-9_$][\w$]*)\s*=\s*(?:async\s*)?\(/,
  )
  if (!hookDeclaration) return undefined
  return {
    name: hookDeclaration[1],
    kind: 'typescript-arrow',
    parameterOpenColumn: hookDeclaration[0].lastIndexOf('('),
  }
}

const matchingDelimiterOffset = (source, start, open, close) => {
  let depth = 0
  for (let index = start; index < source.length; index += 1) {
    if (source[index] === open) depth += 1
    if (source[index] !== close) continue
    depth -= 1
    if (depth === 0) return index
  }
  return undefined
}

const blockBoundaryAfterParameters = (source, start, kind) => {
  let index = skipWhitespace(source, start)
  if (kind !== 'typescript-function' || source[index] !== ':') {
    for (; index < source.length; index += 1) {
      if (source[index] === '{' || source[index] === ';') return index
    }
    return undefined
  }

  index += 1
  let parentheses = 0
  let brackets = 0
  let angles = 0
  let braces = 0
  let hasTypeToken = false
  let penultimateToken = ''
  let previousToken = ''

  for (; index < source.length; index += 1) {
    const character = source[index]
    const atTopLevel = parentheses === 0
      && brackets === 0
      && angles === 0
      && braces === 0

    if (atTopLevel && character === ';') return index
    if (atTopLevel && character === '{') {
      const followsCallableArrow = penultimateToken === '=' && previousToken === '>'
      if (
        !hasTypeToken
        || ['|', '&', '?', ':'].includes(previousToken)
        || followsCallableArrow
      ) {
        braces = 1
        hasTypeToken = true
        penultimateToken = previousToken
        previousToken = character
        continue
      }
      return index
    }

    if (character === '(') parentheses += 1
    if (character === ')') parentheses = Math.max(0, parentheses - 1)
    if (character === '[') brackets += 1
    if (character === ']') brackets = Math.max(0, brackets - 1)
    if (character === '<') angles += 1
    if (character === '>') angles = Math.max(0, angles - 1)
    if (character === '{') braces += 1
    if (character === '}') braces = Math.max(0, braces - 1)

    if (!/\s/.test(character)) {
      hasTypeToken = true
      penultimateToken = previousToken
      previousToken = character
    }
  }
  return undefined
}

const expressionBoundaryOffset = (source, start, limit) => {
  let parentheses = 0
  let brackets = 0
  let braces = 0
  let previousCodeOffset = start

  for (let index = start; index < limit; index += 1) {
    const character = source[index]
    const atTopLevel = parentheses === 0 && brackets === 0 && braces === 0
    if (atTopLevel && character === ';') return index
    if (atTopLevel && (character === '\n' || character === '\r')) {
      return previousCodeOffset
    }

    if (character === '(') parentheses += 1
    if (character === ')') parentheses = Math.max(0, parentheses - 1)
    if (character === '[') brackets += 1
    if (character === ']') brackets = Math.max(0, brackets - 1)
    if (character === '{') braces += 1
    if (character === '}') braces = Math.max(0, braces - 1)
    if (!/\s/.test(character)) previousCodeOffset = index
  }
  return previousCodeOffset
}

const declarationBoundaryOffset = (source, declaration, limit) => {
  const parameterOpen = declaration.parameterOpenOffset
    ?? source.indexOf('(', declaration.headEndOffset)
  if (parameterOpen === -1 || parameterOpen >= limit) return limit - 1

  const parameterClose = matchingDelimiterOffset(source, parameterOpen, '(', ')')
  if (parameterClose === undefined || parameterClose >= limit) return limit - 1

  if (declaration.kind === 'typescript-arrow') {
    const arrow = source.indexOf('=>', parameterClose + 1)
    if (arrow === -1 || arrow >= limit) return limit - 1
    const expressionStart = skipWhitespace(source, arrow + 2)
    if (source[expressionStart] !== '{') {
      return expressionBoundaryOffset(source, expressionStart, limit)
    }
    return matchingDelimiterOffset(source, expressionStart, '{', '}') ?? limit - 1
  }

  const bodyStart = blockBoundaryAfterParameters(
    source,
    parameterClose + 1,
    declaration.kind,
  )
  if (bodyStart === undefined || bodyStart >= limit || source[bodyStart] === ';') {
    return Math.min(bodyStart ?? limit - 1, limit - 1)
  }
  return matchingDelimiterOffset(source, bodyStart, '{', '}') ?? limit - 1
}

const decisionScore = (source, language) => {
  const masked = maskNonCode(source, language)
  const keywordCount = [...masked.matchAll(/\b(?:if|for|while|match|case)\b/g)].length
  const logicalCount = [...masked.matchAll(/&&|\|\|/g)].length
  let ternaryCount = 0

  if (language === 'typescript') {
    for (let index = 0; index < masked.length; index += 1) {
      if (masked[index] !== '?') continue
      const previous = masked[index - 1] ?? ''
      let nextIndex = index + 1
      while (/\s/.test(masked[nextIndex] ?? '')) nextIndex += 1
      const next = masked[nextIndex] ?? ''
      if (previous === '?' || next === '?' || next === '.' || next === ':') continue
      ternaryCount += 1
    }
  }

  return 1 + keywordCount + logicalCount + ternaryCount
}

const compareFunctions = (left, right) =>
  left.path.localeCompare(right.path)
  || left.startLine - right.startLine
  || left.name.localeCompare(right.name)

export function measureFunctions(files) {
  const functions = []
  const { sources, classified } = classifyTestLines(files)

  for (const [path, source] of sources) {
    if (!/\.(?:rs|tsx?)$/.test(path)) continue

    const totalLines = countLines(source)
    const language = path.endsWith('.rs') ? 'rust' : 'typescript'
    const masked = maskNonCode(source, language)
    const lines = masked.split(/\r?\n/).slice(0, totalLines)
    const lineStartOffsets = [
      0,
      ...[...masked.matchAll(/\n/g)].map((lineBreak) => lineBreak.index + 1),
    ]
    const testLines = classified.get(path)
    const declarations = []

    for (let index = 0; index < lines.length; index += 1) {
      const lineStartOffset = lineStartOffsets[index]
      const declaration = declarationFromLine(path, lines[index])
      if (declaration) {
        declarations.push({
          ...declaration,
          index,
          lineStartOffset,
          declarationOffset: lineStartOffset + lines[index].search(/\S/),
          headEndOffset: lineStartOffset + (declaration.headEndColumn ?? 0),
          parameterOpenOffset: declaration.parameterOpenColumn === undefined
            ? undefined
            : lineStartOffset + declaration.parameterOpenColumn,
        })
      }
    }

    for (let index = 0; index < declarations.length; index += 1) {
      const declaration = declarations[index]
      const nextDeclarationOffset = declarations[index + 1]?.lineStartOffset ?? masked.length
      declaration.endOffset = declarationBoundaryOffset(
        masked,
        declaration,
        nextDeclarationOffset,
      )
    }

    for (const declaration of declarations) {
      if (testLines.has(declaration.index)) continue
      const endIndex = lineIndexAtOffset(masked, declaration.endOffset)
      const span = [...masked.slice(declaration.lineStartOffset, declaration.endOffset + 1)]
      for (const nested of declarations) {
        if (
          nested.declarationOffset <= declaration.declarationOffset
          || nested.endOffset > declaration.endOffset
        ) continue
        const nestedStart = nested.declarationOffset - declaration.lineStartOffset
        const nestedEnd = nested.endOffset - declaration.lineStartOffset
        for (let offset = nestedStart; offset <= nestedEnd; offset += 1) {
          if (span[offset] !== '\n' && span[offset] !== '\r') span[offset] = ' '
        }
      }
      functions.push({
        path,
        name: declaration.name,
        startLine: declaration.index + 1,
        lines: endIndex - declaration.index + 1,
        decisionScore: decisionScore(span.join(''), language),
      })
    }
  }

  return {
    functionsOver200Lines: functions
      .filter((entry) => entry.lines > 200)
      .sort(compareFunctions),
    functionsOverDecisionScore15: functions
      .filter((entry) => entry.decisionScore > 15)
      .sort(compareFunctions),
  }
}

const walkSourceDirectory = (root, directory, files) => {
  if (!existsSync(directory)) return
  for (const entry of readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))) {
    if (entry.name === 'target') continue
    const path = join(directory, entry.name)
    if (entry.isDirectory()) {
      walkSourceDirectory(root, path, files)
      continue
    }
    if (!entry.isFile() || !['.rs', '.ts', '.tsx'].includes(extname(entry.name))) continue
    files.set(normalizePath(relative(root, path)), readFileSync(path, 'utf8'))
  }
}

export function collectSourceFiles(root) {
  const absoluteRoot = resolve(root)
  const sourceRoots = []
  const cratesDirectory = join(absoluteRoot, 'crates')
  if (existsSync(cratesDirectory)) {
    for (const crate of readdirSync(cratesDirectory, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .sort((left, right) => left.name.localeCompare(right.name))) {
      sourceRoots.push(join(cratesDirectory, crate.name, 'src'))
    }
  }
  sourceRoots.push(join(absoluteRoot, 'src-tauri', 'src'))
  sourceRoots.push(join(absoluteRoot, 'ui', 'src'))

  const files = new Map()
  for (const sourceRoot of sourceRoots) walkSourceDirectory(absoluteRoot, sourceRoot, files)
  return new Map([...files].sort(([left], [right]) => left.localeCompare(right)))
}

export function collectArchitectureTrends(root) {
  const files = collectSourceFiles(root)
  return {
    schemaVersion: SCHEMA_VERSION,
    ...measureSourceFiles(files),
    ...measureFunctions(files),
  }
}

const outlierKey = (entry) => `${entry.path}\0${entry.name ?? ''}`

export function compareArchitectureTrends(current, baseline) {
  const warnings = []
  const baselineFiles = new Set(baseline.filesOver1000Lines.map(({ path }) => path))
  const baselineLongFunctions = new Set(baseline.functionsOver200Lines.map(outlierKey))
  const baselineComplexFunctions = new Set(
    baseline.functionsOverDecisionScore15.map(outlierKey),
  )

  for (const file of [...current.filesOver1000Lines]
    .sort((left, right) => left.path.localeCompare(right.path))) {
    if (!baselineFiles.has(file.path)) {
      warnings.push(`new production file over 1000 lines: ${file.path} (${file.lines})`)
    }
  }
  for (const fn of [...current.functionsOver200Lines].sort(compareFunctions)) {
    if (!baselineLongFunctions.has(outlierKey(fn))) {
      warnings.push(
        `new function over 200 lines: ${fn.path}:${fn.name} at line ${fn.startLine} (${fn.lines})`,
      )
    }
  }
  for (const fn of [...current.functionsOverDecisionScore15].sort(compareFunctions)) {
    if (!baselineComplexFunctions.has(outlierKey(fn))) {
      warnings.push(
        `new function over decision score 15: ${fn.path}:${fn.name} at line ${fn.startLine} (${fn.decisionScore})`,
      )
    }
  }
  if (current.testToProductionRatio < baseline.testToProductionRatio - 0.02) {
    warnings.push(
      `test-to-production ratio dropped from ${baseline.testToProductionRatio} `
      + `to ${current.testToProductionRatio}`,
    )
  }
  return warnings
}

const classificationKey = ({ metric, path, symbol = '' }) =>
  `${metric}\0${path}\0${symbol}`

const ALLOWED_TREND_CLASSIFICATIONS = new Set([
  'accepted',
  'governance-target',
  'test-exception',
])

const baselineClassificationKeys = (baseline) => [
  ...baseline.filesOver1000Lines.map(({ path }) => classificationKey({
    metric: 'file-over-1000',
    path,
  })),
  ...baseline.functionsOver200Lines.map(({ path, name }) => classificationKey({
    metric: 'function-over-200',
    path,
    symbol: name,
  })),
  ...baseline.functionsOverDecisionScore15.map(({ path, name }) => classificationKey({
    metric: 'function-decision-score',
    path,
    symbol: name,
  })),
].sort()

export function validateTrendClassifications(baseline, registry) {
  if (baseline?.schemaVersion !== SCHEMA_VERSION) {
    return [`trend baseline schema must be version ${SCHEMA_VERSION}`]
  }
  if (
    !Array.isArray(baseline.filesOver1000Lines)
    || !Array.isArray(baseline.functionsOver200Lines)
    || !Array.isArray(baseline.functionsOverDecisionScore15)
  ) {
    return ['trend baseline must contain source metric arrays']
  }
  if (registry?.schemaVersion !== SCHEMA_VERSION || !Array.isArray(registry.entries)) {
    return [`trend classification schema must be version ${SCHEMA_VERSION}`]
  }

  const errors = []
  const baselineKeys = new Set(baselineClassificationKeys(baseline))
  const keys = new Set()
  for (const entry of registry.entries) {
    const key = classificationKey(entry)
    if (keys.has(key)) errors.push(`duplicate classification: ${key}`)
    keys.add(key)
    if (!ALLOWED_TREND_CLASSIFICATIONS.has(entry.classification)) {
      errors.push(`unknown classification: ${key}`)
    }
    for (const field of ['owner', 'rationale', 'reviewTrigger']) {
      if (typeof entry[field] !== 'string' || entry[field].trim() === '') {
        errors.push(`blank ${field}: ${key}`)
      }
    }
    if (!baselineKeys.has(key)) {
      errors.push(`classification without baseline outlier: ${key}`)
    }
  }
  for (const key of baselineKeys) {
    if (!keys.has(key)) errors.push(`missing classification: ${key}`)
  }
  return errors
}

const readJsonFile = (path) => JSON.parse(readFileSync(path, 'utf8'))

export function runArchitectureTrendsCli(
  argv,
  {
    root = process.cwd(),
    collect = collectArchitectureTrends,
    readJson = readJsonFile,
    stdout = process.stdout,
    stderr = process.stderr,
  } = {},
) {
  const [mode, baselineArgument, classificationsArgument] = argv
  if (mode === '--report' && argv.length === 1) {
    stdout.write(`${JSON.stringify(collect(root), null, 2)}\n`)
    return 0
  }
  if (
    mode !== '--check'
    || !baselineArgument
    || !classificationsArgument
    || argv.length !== 3
  ) {
    throw new Error(
      'usage: architecture-trends.mjs --report | --check baseline classifications',
    )
  }

  let baseline
  let registry
  try {
    baseline = readJson(resolve(root, baselineArgument))
    registry = readJson(resolve(root, classificationsArgument))
  } catch (error) {
    stderr.write(`ERROR: ${error instanceof Error ? error.message : String(error)}\n`)
    return 1
  }

  const governanceErrors = validateTrendClassifications(baseline, registry)
  if (governanceErrors.length > 0) {
    for (const error of governanceErrors) stderr.write(`ERROR: ${error}\n`)
    return 1
  }

  const warnings = compareArchitectureTrends(collect(root), baseline)
  for (const warning of warnings) {
    stderr.write(`WARNING: unclassified trend finding: ${warning}\n`)
  }
  stdout.write(
    warnings.length === 0
      ? 'Architecture trend check passed.\n'
      : `Architecture trend check completed with ${warnings.length} warning(s).\n`,
  )
  return 0
}

if (
  process.argv[1]
  && import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  process.exitCode = runArchitectureTrendsCli(process.argv.slice(2))
}
