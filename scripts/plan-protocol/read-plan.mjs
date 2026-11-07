#!/usr/bin/env node
import { pathToFileURL } from 'node:url'
import { invokePlanReader } from './reader.mjs'

export function parseArguments(args) {
  const result = {}
  for (let index = 0; index < args.length; index += 1) {
    const key = { '--project': 'projectRoot', '--plan': 'planId' }[args[index]]
    if (!key || Object.hasOwn(result, key)) throw new Error('unknown or duplicate plan reader argument')
    const value = args[++index]
    if (value === undefined) throw new Error(`${args[index - 1]} requires a value`)
    result[key] = value
  }
  if (!result.projectRoot) throw new Error('--project is required')
  return result
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const options = parseArguments(process.argv.slice(2))
    const operation = options.planId ? 'current' : 'list'
    const result = await invokePlanReader(operation, options)
    process.stdout.write(`${JSON.stringify(result)}\n`)
  } catch (error) {
    process.stderr.write(`${JSON.stringify({ status: 'error', code: error.code ?? 'io', message: error.message })}\n`)
    process.exitCode = 1
  }
}
