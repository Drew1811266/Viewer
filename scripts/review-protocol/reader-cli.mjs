export function readError(error) {
  const protocolVersion = error.protocolVersion === 'viewer.review/4' ? error.protocolVersion : 'viewer.review/3'
  return { protocolVersion, status: 'error', code: error.code ?? 'io',
    message: error instanceof Error ? error.message : 'review reader failed' }
}

export function parseArguments(args, { history = false } = {}) {
  const keys = new Map(history
    ? [['--project', 'projectRoot'], ['--stream', 'reviewStreamId'], ['--snapshot', 'snapshotId'], ['--archive', 'archiveId'], ['--legacy-round', 'legacyRoundId']]
    : [['--project', 'projectRoot'], ['--stream', 'reviewStreamId'], ['--task', 'taskId'], ['--batch', 'batchId'], ['--since', 'sinceSnapshotId']])
  const result = {}
  for (let i = 0; i < args.length; i += 1) {
    if (!history && args[i] === '--list' && result.list === undefined) { result.list = true; continue }
    const key = keys.get(args[i])
    if (!key || Object.hasOwn(result, key)) throw Object.assign(new Error('unknown or duplicate reader argument'), { code: 'integrity' })
    const value = args[++i]
    if (value === undefined || value.startsWith('--')) throw Object.assign(new Error('reader argument requires a value'), { code: 'integrity' })
    result[key] = value
  }
  if (!result.projectRoot) throw Object.assign(new Error('--project is required'), { code: 'integrity' })
  return result
}

export async function runCli(read, args, { history = false, list } = {}) {
  let result
  try {
    const { list: listing, ...options } = parseArguments(args, { history })
    result = await (listing ? list(options) : read(options))
  } catch (error) { result = readError(error) }
  const failed = result.status === 'error'
  const output = failed ? process.stderr : process.stdout
  output.write(`${JSON.stringify(result)}\n`)
  return failed ? 1 : 0
}
