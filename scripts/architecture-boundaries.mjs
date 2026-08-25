#!/usr/bin/env node

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { dirname, extname, join, relative, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

export const PRODUCTION_DEPENDENCY_TARGETS = new Map([
  ['viewer-domain', new Set()],
  ['viewer-application', new Set(['viewer-domain'])],
  ['viewer-infrastructure', new Set([
    'viewer-application',
    'viewer-domain',
    'viewer-video-mpv',
  ])],
  ['viewer-platform-macos', new Set([
    'viewer-application',
    'viewer-domain',
    'viewer-video-mpv',
  ])],
  ['viewer-video-mpv', new Set()],
  ['viewer-test-support', new Set(['viewer-application', 'viewer-domain'])],
  ['viewer-desktop', new Set([
    'viewer-application',
    'viewer-domain',
    'viewer-infrastructure',
    'viewer-platform-macos',
    'viewer-video-mpv',
  ])],
])

const compareEdges = (left, right) =>
  left.from.localeCompare(right.from)
  || left.to.localeCompare(right.to)
  || left.kind.localeCompare(right.kind)

const normalizePath = (path) => path.replaceAll('\\', '/')

const isProductionUiPath = (path) =>
  path.startsWith('ui/src/')
  && !path.startsWith('ui/src/acceptance/')
  && !/\.test\.tsx?$/.test(path)

const isTauriSpecifier = (specifier) =>
  specifier === '@tauri-apps/api'
  || specifier.startsWith('@tauri-apps/api/')
  || specifier.startsWith('@tauri-apps/plugin-')

const ALLOWED_TAURI_UI_FILES = new Set(['ui/src/api/viewer.ts'])

export function readCargoMetadata(root, execFile = execFileSync) {
  return JSON.parse(execFile(
    'cargo',
    ['metadata', '--locked', '--no-deps', '--format-version', '1'],
    {
      cwd: root,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  ))
}

export function collectWorkspaceDependencyEdges(metadata) {
  const memberIds = new Set(metadata.workspace_members ?? [])
  const members = (metadata.packages ?? []).filter(
    (pkg) => memberIds.has(pkg.id) && pkg.name.startsWith('viewer-'),
  )
  const memberPaths = new Map(members.map((pkg) => [
    pkg.name,
    resolve(dirname(pkg.manifest_path)),
  ]))
  const edges = []

  for (const pkg of members) {
    for (const dependency of pkg.dependencies ?? []) {
      const memberPath = memberPaths.get(dependency.name)
      if (
        !memberPath
        || dependency.source !== null
        || !dependency.path
        || resolve(dependency.path) !== memberPath
      ) continue
      edges.push({
        from: pkg.name,
        to: dependency.name,
        kind: dependency.kind ?? 'normal',
      })
    }
  }

  return edges.sort(compareEdges)
}

const walkUiDirectory = (root, directory, files) => {
  if (!existsSync(directory)) return
  for (const entry of readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))) {
    const absolutePath = join(directory, entry.name)
    const path = normalizePath(relative(root, absolutePath))
    if (entry.isDirectory()) {
      if (path !== 'ui/src/acceptance') walkUiDirectory(root, absolutePath, files)
      continue
    }
    if (
      entry.isFile()
      && ['.ts', '.tsx'].includes(extname(entry.name))
      && isProductionUiPath(path)
    ) {
      files.set(path, readFileSync(absolutePath, 'utf8'))
    }
  }
}

export function collectProductionUiFiles(root) {
  const absoluteRoot = resolve(root)
  const files = new Map()
  walkUiDirectory(absoluteRoot, join(absoluteRoot, 'ui', 'src'), files)
  return new Map([...files].sort(([left], [right]) => left.localeCompare(right)))
}

const skipQuoted = (source, start) => {
  const quote = source[start]
  let value = ''
  let index = start + 1
  while (index < source.length) {
    if (source[index] === '\\') {
      value += source[index + 1] ?? ''
      index += 2
      continue
    }
    if (source[index] === quote) return { value, next: index + 1 }
    value += source[index]
    index += 1
  }
  return { value, next: source.length }
}

const skipTrivia = (source, start) => {
  let index = start
  while (index < source.length) {
    if (/\s/.test(source[index])) {
      index += 1
      continue
    }
    if (source.startsWith('//', index)) {
      const newline = source.indexOf('\n', index + 2)
      index = newline === -1 ? source.length : newline + 1
      continue
    }
    if (source.startsWith('/*', index)) {
      const close = source.indexOf('*/', index + 2)
      index = close === -1 ? source.length : close + 2
      continue
    }
    break
  }
  return index
}

const isIdentifierCharacter = (value) => /[\w$]/.test(value ?? '')

const readIdentifier = (source, start) => {
  let index = start
  while (isIdentifierCharacter(source[index])) index += 1
  return { value: source.slice(start, index), next: index }
}

const collectImportSpecifiers = (source) => {
  const specifiers = []
  let index = 0
  while (index < source.length) {
    index = skipTrivia(source, index)
    if (index >= source.length) break
    if (['"', "'", '`'].includes(source[index])) {
      index = skipQuoted(source, index).next
      continue
    }
    if (!isIdentifierCharacter(source[index])) {
      index += 1
      continue
    }

    const token = readIdentifier(source, index)
    index = token.next
    if (token.value !== 'import') continue

    let cursor = skipTrivia(source, index)
    if (source[cursor] === '.') continue
    if (source[cursor] === '(') {
      cursor = skipTrivia(source, cursor + 1)
      if (source[cursor] === '"' || source[cursor] === "'") {
        specifiers.push(skipQuoted(source, cursor).value)
      }
      continue
    }
    if (source[cursor] === '"' || source[cursor] === "'") {
      specifiers.push(skipQuoted(source, cursor).value)
      continue
    }

    while (cursor < source.length && source[cursor] !== ';') {
      cursor = skipTrivia(source, cursor)
      if (source[cursor] === '"' || source[cursor] === "'" || source[cursor] === '`') {
        cursor = skipQuoted(source, cursor).next
        continue
      }
      if (!isIdentifierCharacter(source[cursor])) {
        cursor += 1
        continue
      }
      const clauseToken = readIdentifier(source, cursor)
      cursor = clauseToken.next
      if (clauseToken.value !== 'from') continue
      cursor = skipTrivia(source, cursor)
      if (source[cursor] === '"' || source[cursor] === "'") {
        specifiers.push(skipQuoted(source, cursor).value)
      }
      break
    }
    index = cursor
  }
  return specifiers
}

export function findForbiddenTauriImports(files) {
  const violations = new Map()
  for (const [rawPath, source] of files) {
    const path = normalizePath(rawPath)
    if (!isProductionUiPath(path) || ALLOWED_TAURI_UI_FILES.has(path)) continue
    for (const specifier of collectImportSpecifiers(source)) {
      if (!isTauriSpecifier(specifier)) continue
      violations.set(`${path}\0${specifier}`, { path, specifier })
    }
  }
  return [...violations.values()].sort(
    (left, right) => left.path.localeCompare(right.path)
      || left.specifier.localeCompare(right.specifier),
  )
}

export function findForbiddenWorkspaceEdges(edges) {
  const knownPackages = new Set(PRODUCTION_DEPENDENCY_TARGETS.keys())
  return edges
    .filter((edge) => {
      if (edge.kind === 'dev') return false
      const allowed = PRODUCTION_DEPENDENCY_TARGETS.get(edge.from)
      return allowed === undefined || !knownPackages.has(edge.to) || !allowed.has(edge.to)
    })
    .sort(compareEdges)
}

const compareCycles = (left, right) => left.join('\0').localeCompare(right.join('\0'))

export function findWorkspaceDependencyCycles(edges) {
  const adjacency = new Map()
  for (const edge of edges.filter(({ kind }) => kind !== 'dev').sort(compareEdges)) {
    if (!adjacency.has(edge.from)) adjacency.set(edge.from, new Set())
    if (!adjacency.has(edge.to)) adjacency.set(edge.to, new Set())
    adjacency.get(edge.from).add(edge.to)
  }

  const cycles = new Map()
  const nodes = [...adjacency.keys()].sort()
  for (const start of nodes) {
    const visit = (node, path, visited) => {
      const neighbors = [...(adjacency.get(node) ?? [])].sort()
      for (const neighbor of neighbors) {
        if (neighbor === start) {
          const cycle = [...path, start]
          cycles.set(cycle.join('\0'), cycle)
          continue
        }
        if (neighbor.localeCompare(start) < 0 || visited.has(neighbor)) continue
        visit(neighbor, [...path, neighbor], new Set([...visited, neighbor]))
      }
    }
    visit(start, [start], new Set([start]))
  }

  return [...cycles.values()].sort(compareCycles)
}

export function runArchitectureBoundariesCli({
  root = process.cwd(),
  readMetadata = readCargoMetadata,
  collectUiFiles = collectProductionUiFiles,
  stdout = process.stdout,
  stderr = process.stderr,
} = {}) {
  const edges = collectWorkspaceDependencyEdges(readMetadata(root))
  const forbiddenEdges = findForbiddenWorkspaceEdges(edges)
  const cycles = findWorkspaceDependencyCycles(edges)
  const forbiddenImports = findForbiddenTauriImports(collectUiFiles(root))

  for (const edge of forbiddenEdges) {
    stderr.write(
      `ERROR: forbidden workspace dependency: ${edge.from} -> ${edge.to} (${edge.kind})\n`,
    )
  }
  for (const cycle of cycles) {
    stderr.write(`ERROR: workspace dependency cycle: ${cycle.join(' -> ')}\n`)
  }
  for (const violation of forbiddenImports) {
    stderr.write(
      `ERROR: forbidden production UI Tauri import: ${violation.path} -> ${violation.specifier}\n`,
    )
  }
  if (forbiddenEdges.length > 0 || cycles.length > 0 || forbiddenImports.length > 0) {
    return 1
  }

  stdout.write('Architecture boundary check passed.\n')
  return 0
}

if (
  process.argv[1]
  && import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  process.exitCode = runArchitectureBoundariesCli()
}
