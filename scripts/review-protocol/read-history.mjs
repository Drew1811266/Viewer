#!/usr/bin/env node
import { pathToFileURL } from 'node:url'
import { invokeReader } from './native-reader.mjs'
import { readError, runCli } from './reader-cli.mjs'

export async function readReviewHistory({ projectRoot, reviewStreamId, snapshotId, archiveId, legacyRoundId } = {}, control = {}) {
  try { return await invokeReader('history', { projectRoot, reviewStreamId, snapshotId, archiveId, legacyRoundId }, control) }
  catch (error) { return readError(error) }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await runCli(readReviewHistory, process.argv.slice(2), { history: true })
}
