#!/usr/bin/env node

import { execFileSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
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
  stdout = process.stdout,
  stderr = process.stderr,
} = {}) {
  const edges = collectWorkspaceDependencyEdges(readMetadata(root))
  const forbiddenEdges = findForbiddenWorkspaceEdges(edges)
  const cycles = findWorkspaceDependencyCycles(edges)

  for (const edge of forbiddenEdges) {
    stderr.write(
      `ERROR: forbidden workspace dependency: ${edge.from} -> ${edge.to} (${edge.kind})\n`,
    )
  }
  for (const cycle of cycles) {
    stderr.write(`ERROR: workspace dependency cycle: ${cycle.join(' -> ')}\n`)
  }
  if (forbiddenEdges.length > 0 || cycles.length > 0) return 1

  stdout.write('Architecture boundary check passed.\n')
  return 0
}

if (
  process.argv[1]
  && import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  process.exitCode = runArchitectureBoundariesCli()
}
