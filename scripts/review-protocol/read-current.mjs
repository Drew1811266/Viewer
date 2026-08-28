#!/usr/bin/env node
import { pathToFileURL } from 'node:url'
import { invokeReader } from './native-reader.mjs'
import { readError, runCli } from './reader-cli.mjs'

export async function readCurrentReview({ projectRoot, reviewStreamId, taskId, batchId, sinceSnapshotId } = {}, control = {}) {
  try { return await invokeReader('current', { projectRoot, reviewStreamId, taskId, batchId, sinceSnapshotId }, control) }
  catch (error) { return readError(error) }
}

export async function listCurrentReviewStreams(options = {}, control = {}) {
  try { return await invokeReader('list', options, control) }
  catch (error) { return readError(error) }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await runCli(readCurrentReview, process.argv.slice(2), { list: listCurrentReviewStreams })
}
