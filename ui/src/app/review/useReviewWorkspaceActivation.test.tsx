import { act, renderHook, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type { ReviewAuthoringApplyResult } from '../../api/reviewWorkspaceTypes'
import { reviewWorkspaceError } from './continuousReviewModel'
import {
  applied,
  deferred,
  input,
  reviewPort,
  session,
  workspace,
} from './continuousReviewTestFixtures'
import { useReviewWorkspaceActivation } from './useReviewWorkspaceActivation'

it('keeps the initialized workbench presentation mounted while a save is in flight', async () => {
  const port = reviewPort(workspace('base'))
  const applying = deferred<ReviewAuthoringApplyResult>()
  vi.mocked(port.applyCommand).mockReturnValueOnce(applying.promise)
  const { result } = renderHook(() => useReviewWorkspaceActivation({ port, ...session }))
  await waitFor(() => expect(result.current.presentation).toBeDefined())
  act(() => {
    expect(result.current.coordinator?.beginEditor(input)).toBe(true)
  })

  let saving!: Promise<void>
  act(() => {
    saving = result.current.coordinator?.saveFeedback() ?? Promise.resolve()
  })
  await waitFor(() => expect(port.applyCommand).toHaveBeenCalledOnce())
  expect(result.current.coordinator?.state.kind).toBe('saving_authoring')
  const presentationStayedMounted = result.current.presentation === result.current.coordinator
  const envelope = vi.mocked(port.applyCommand).mock.calls[0]?.[0].envelope
  if (!envelope) throw new Error('Missing prepared save envelope')

  await act(async () => {
    applying.resolve(applied(envelope))
    await saving
  })

  expect(presentationStayedMounted).toBe(true)
  expect(result.current.presentation).toBe(result.current.coordinator)
})

it('retries a transient initial workspace read without exposing an unavailable state', async () => {
  const port = reviewPort(workspace('base'))
  vi.mocked(port.getWorkspace)
    .mockRejectedValueOnce(reviewWorkspaceError('internal', 'worker starting', true))
    .mockResolvedValueOnce(workspace('base'))

  const { result } = renderHook(() => useReviewWorkspaceActivation({ port, ...session }))

  await waitFor(() => expect(result.current.presentation).toBeDefined())
  expect(port.getWorkspace).toHaveBeenCalledTimes(2)
  expect(result.current.coordinator?.state.kind).toBe('ready')
})
