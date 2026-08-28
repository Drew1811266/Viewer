#!/usr/bin/env node

import { pathToFileURL } from 'node:url'
import { invokeReader } from './native-reader.mjs'
export { blake3Hex } from './blake3.mjs'

export async function listReviewStreams(options = {}) {
  return invokeReader('legacyList', options)
}

export async function readLatestCompletedReview(options = {}) {
  return invokeReader('legacyLatest', options)
}

function parseArguments(argumentsList) {
  const options = {}
  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index]
    if (argument === '--list') {
      if (options.list) throw new Error('--list may be provided only once')
      options.list = true
      continue
    }
    const keys = new Map([
      ['--project', 'projectRoot'],
      ['--stream', 'reviewStreamId'],
      ['--task', 'taskId'],
      ['--batch', 'batchId'],
    ])
    const key = keys.get(argument)
    if (!key) throw new Error(`unknown argument: ${argument}`)
    if (options[key] !== undefined) throw new Error(`${argument} may be provided only once`)
    const value = argumentsList[index + 1]
    if (value === undefined || value.startsWith('--')) throw new Error(`${argument} requires a value`)
    options[key] = value
    index += 1
  }
  if (options.projectRoot === undefined) throw new Error('--project is required')
  if (options.list && (options.reviewStreamId || options.taskId || options.batchId)) {
    throw new Error('--list cannot be combined with a Stream selector')
  }
  return options
}

async function main(argumentsList) {
  try {
    const { list, ...options } = parseArguments(argumentsList)
    const result = list
      ? await listReviewStreams(options)
      : await readLatestCompletedReview(options)
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
    return 0
  } catch (error) {
    process.stderr.write(`error: ${error instanceof Error ? error.message : 'review reader failed'}\n`)
    return 1
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await main(process.argv.slice(2))
}
