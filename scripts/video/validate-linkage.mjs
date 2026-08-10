import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'

const allowedPrefixes = [
  '@executable_path/',
  '@loader_path/',
  '@rpath/',
  '/System/Library/Frameworks/',
  '/usr/lib/',
]

export function validateLinkage(output) {
  const dependencies = output
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.includes(' (compatibility version'))
    .map((line) => line.slice(0, line.indexOf(' (compatibility version')))

  if (dependencies.length === 0) {
    throw new Error('linkage report contains no dynamic dependencies')
  }

  for (const dependency of dependencies) {
    if (!allowedPrefixes.some((prefix) => dependency.startsWith(prefix))) {
      throw new Error(`prohibited dynamic dependency: ${dependency}`)
    }
  }
}

async function main([path]) {
  if (!path) {
    throw new Error('usage: validate-linkage.mjs <otool-linkage.txt>')
  }
  validateLinkage(await readFile(path, 'utf8'))
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`${error.message}\n`)
    process.exitCode = 1
  })
}
