import { useEffect, useRef, useState } from 'react'
import type { BrowserFile, ImageRepresentation, ImageRepresentationRequest } from '../../api/types'

export type CurrentOriginalStatus = 'idle' | 'loading' | 'ready' | 'budget_error' | 'error'

export interface CurrentOriginalState {
  status: CurrentOriginalStatus
  entityId: string
  representation: ImageRepresentation | null
}

interface CurrentOriginalOptions {
  file: BrowserFile
  needed: boolean
  available: boolean
  requestImage: (
    file: BrowserFile,
    representation: ImageRepresentationRequest,
    signal?: AbortSignal,
  ) => Promise<ImageRepresentation>
}

export function useCurrentOriginal({
  file,
  needed,
  available,
  requestImage,
}: CurrentOriginalOptions): CurrentOriginalState {
  const currentFile = useRef(file)
  const revision = useRef(0)
  currentFile.current = file
  const [state, setState] = useState<CurrentOriginalState>(() => idle(file.entityId))

  useEffect(() => {
    const requestRevision = ++revision.current
    const entityId = file.entityId
    if (!needed || !available) {
      setState(idle(entityId))
      return
    }

    const controller = new AbortController()
    setState({ status: 'loading', entityId, representation: null })
    let request: Promise<ImageRepresentation>
    try {
      request = requestImage(
        currentFile.current,
        { kind: 'original100_percent' },
        controller.signal,
      )
    } catch (caught) {
      settleFailure(caught, controller.signal, requestRevision, revision, entityId, setState)
      return () => controller.abort()
    }
    void request.then(
      (representation) => {
        if (revision.current !== requestRevision || controller.signal.aborted) return
        setState({ status: 'ready', entityId, representation })
      },
      (caught: unknown) => {
        settleFailure(caught, controller.signal, requestRevision, revision, entityId, setState)
      },
    )
    return () => {
      controller.abort()
      if (revision.current === requestRevision) revision.current += 1
    }
  }, [available, file.entityId, needed, requestImage])

  return state
}

function idle(entityId: string): CurrentOriginalState {
  return { status: 'idle', entityId, representation: null }
}

function settleFailure(
  caught: unknown,
  signal: AbortSignal,
  requestRevision: number,
  revision: { current: number },
  entityId: string,
  setState: (state: CurrentOriginalState) => void,
) {
  if (revision.current !== requestRevision || signal.aborted) return
  if (caught instanceof DOMException && caught.name === 'AbortError') {
    setState(idle(entityId))
    return
  }
  setState({
    status: commandCode(caught) === 'image_budget_exceeded' ? 'budget_error' : 'error',
    entityId,
    representation: null,
  })
}

function commandCode(caught: unknown): string | null {
  if (typeof caught !== 'object' || caught === null || !('code' in caught)) return null
  return typeof caught.code === 'string' ? caught.code : null
}
