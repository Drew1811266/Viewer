#!/usr/bin/env node
import { pathToFileURL } from 'node:url'
import { invokePlanReader } from './reader.mjs'
import { parseArguments } from './read-plan.mjs'

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const options = parseArguments(process.argv.slice(2))
    if (!options.planId) throw new Error('--plan is required for marks')
    const result = await invokePlanReader('marks', options)
    process.stdout.write(`${JSON.stringify(result)}\n`)
  } catch (error) {
    process.stderr.write(`${JSON.stringify({ status: 'error', code: error.code ?? 'io', message: error.message })}\n`)
    process.exitCode = 1
  }
}
