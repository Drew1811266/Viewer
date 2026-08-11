function hasExactExecutable(processInfo, executablePath) {
  const command = processInfo.command.trim()
  return command === executablePath || command.startsWith(`${executablePath} `)
}

export const renderFeasibilityTestPaths = Object.freeze([
  'scripts/video/render-feasibility-assertions.test.mjs',
  'scripts/video/render-feasibility-lifecycle.test.mjs',
])

export function attributableViewerProcesses(processes, executablePath, baselinePids) {
  return processes.filter(
    (processInfo) =>
      !baselinePids.has(processInfo.pid) && hasExactExecutable(processInfo, executablePath),
  )
}

function failureText(error) {
  return error?.stack ?? String(error)
}

export function appendCleanupFailure(primaryFailure, cleanupError) {
  const cleanupFailure = `Cleanup failure: ${failureText(cleanupError)}`
  return primaryFailure ? `${primaryFailure}\n${cleanupFailure}` : cleanupFailure
}

async function settleWithin(operation, timeoutMs) {
  let timer
  const operationResult = Promise.resolve()
    .then(operation)
    .then(
      () => ({ status: 'fulfilled' }),
      (error) => ({ status: 'rejected', error }),
    )
  const timeoutResult = new Promise((resolve) => {
    timer = setTimeout(() => resolve({ status: 'timeout' }), timeoutMs)
  })
  const result = await Promise.race([operationResult, timeoutResult])
  clearTimeout(timer)
  return result
}

async function stopClientWithin(client, timeoutMs) {
  const closeResult = await settleWithin(() => client.close(), timeoutMs)
  if (closeResult.status === 'fulfilled') return

  const terminateResult = await settleWithin(() => client.terminate(), timeoutMs)
  if (terminateResult.status === 'fulfilled') return

  const errors = []
  if (closeResult.status === 'rejected') errors.push(closeResult.error)
  if (terminateResult.status === 'rejected') errors.push(terminateResult.error)
  const detail = errors.length > 0 ? `: ${errors.map(failureText).join('; ')}` : ''
  throw new Error(`Native acceptance helper did not stop within ${timeoutMs} ms${detail}`)
}

export async function cleanupFeasibilityLaunch(
  launchState,
  {
    clientTimeoutMs = 3_000,
    currentProcessTable,
    stopLauncher,
    stopProcess,
  },
) {
  const errors = []
  const processErrors = new Map()
  const scanErrors = []
  const stoppedPids = new Set()

  const stopAttributedPid = async (pid) => {
    if (stoppedPids.has(pid)) return
    try {
      await stopProcess(pid)
      stoppedPids.add(pid)
      processErrors.delete(pid)
    } catch (error) {
      processErrors.set(pid, error)
    }
  }

  const rescanAndStop = async () => {
    let processes
    try {
      processes = currentProcessTable()
    } catch (error) {
      scanErrors.push(error)
      return false
    }

    for (const processInfo of attributableViewerProcesses(
      processes,
      launchState.executablePath,
      launchState.baselinePids,
    )) {
      await stopAttributedPid(processInfo.pid)
    }
    return true
  }

  if (launchState.pid) await stopAttributedPid(launchState.pid)
  await rescanAndStop()
  if (launchState.launcher) {
    try {
      await stopLauncher(launchState.launcher)
    } catch (error) {
      errors.push(error)
    }
  }
  const finalScanSucceeded = await rescanAndStop()

  if (!finalScanSucceeded && scanErrors.length > 0) {
    errors.push(new AggregateError(scanErrors, 'Unable to rescan attributable Viewer processes'))
  }

  errors.push(...processErrors.values())

  if (launchState.client) {
    try {
      await stopClientWithin(launchState.client, clientTimeoutMs)
    } catch (error) {
      errors.push(error)
    }
  }

  if (errors.length > 0) {
    throw new AggregateError(
      errors,
      `Native feasibility cleanup failed: ${errors.map(failureText).join('; ')}`,
    )
  }
}
