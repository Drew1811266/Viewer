import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type { ThumbnailDensity, ViewerSettings } from '../api/types'
import { useViewerSettings, ViewerSettingsProvider } from './ViewerSettingsProvider'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((next, fail) => {
    resolve = next
    reject = fail
  })
  return { promise, resolve, reject }
}

function settings(thumbnailDensity: ThumbnailDensity): ViewerSettings {
  return { schemaVersion: 1, thumbnailDensity }
}

function Consumer() {
  const current = useViewerSettings()
  return (
    <>
      <output aria-label="density">{current.thumbnailDensity}</output>
      <output aria-label="height">{current.thumbnailHeight}</output>
      {current.settingsError && <p role="alert">{current.settingsError}</p>}
      {(['compact', 'standard', 'large', 'extra_large', 'maximum'] as const).map((density) => (
        <button key={density} type="button" onClick={() => current.setThumbnailDensity(density)}>
          {density}
        </button>
      ))}
    </>
  )
}

function renderProvider(
  bridge: {
    getViewerSettings: () => Promise<ViewerSettings>
    updateThumbnailDensity: (density: ThumbnailDensity) => Promise<ViewerSettings>
  },
  children: ReactNode = <Consumer />,
) {
  return render(<ViewerSettingsProvider bridge={bridge}>{children}</ViewerSettingsProvider>)
}

describe('ViewerSettingsProvider', () => {
  it('shows standard at first load and adopts a compact backend value', async () => {
    const loaded = deferred<ViewerSettings>()
    renderProvider({
      getViewerSettings: () => loaded.promise,
      updateThumbnailDensity: vi.fn(),
    })

    expect(screen.getByLabelText('density')).toHaveTextContent('standard')
    expect(screen.getByLabelText('height')).toHaveTextContent('132')

    loaded.resolve(settings('compact'))
    await waitFor(() => expect(screen.getByLabelText('density')).toHaveTextContent('compact'))
    expect(screen.getByLabelText('height')).toHaveTextContent('96')
  })

  it('keeps standard without a blocking error when initial loading fails', async () => {
    const loaded = deferred<ViewerSettings>()
    renderProvider({
      getViewerSettings: () => loaded.promise,
      updateThumbnailDensity: vi.fn(),
    })

    await act(async () => loaded.reject(new Error('offline')))
    expect(screen.getByLabelText('density')).toHaveTextContent('standard')
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('publishes maximum immediately before its save resolves', () => {
    const save = deferred<ViewerSettings>()
    renderProvider({
      getViewerSettings: vi.fn().mockResolvedValue(settings('standard')),
      updateThumbnailDensity: () => save.promise,
    })

    fireEvent.click(screen.getByRole('button', { name: 'maximum' }))

    expect(screen.getByLabelText('density')).toHaveTextContent('maximum')
    expect(screen.getByLabelText('height')).toHaveTextContent('240')
  })

  it('serializes extra-large then maximum and ignores the stale completion', async () => {
    const extraLargeSave = deferred<ViewerSettings>()
    const maximumSave = deferred<ViewerSettings>()
    const updateThumbnailDensity = vi
      .fn<(density: ThumbnailDensity) => Promise<ViewerSettings>>()
      .mockImplementationOnce(() => extraLargeSave.promise)
      .mockImplementationOnce(() => maximumSave.promise)
    renderProvider({
      getViewerSettings: vi.fn().mockResolvedValue(settings('standard')),
      updateThumbnailDensity,
    })

    fireEvent.click(screen.getByRole('button', { name: 'extra_large' }))
    fireEvent.click(screen.getByRole('button', { name: 'maximum' }))
    expect(screen.getByLabelText('density')).toHaveTextContent('maximum')
    await waitFor(() => expect(updateThumbnailDensity).toHaveBeenCalledTimes(1))
    expect(updateThumbnailDensity).toHaveBeenNthCalledWith(1, 'extra_large')

    await act(async () => extraLargeSave.resolve(settings('extra_large')))
    expect(screen.getByLabelText('density')).toHaveTextContent('maximum')
    await waitFor(() => expect(updateThumbnailDensity).toHaveBeenCalledTimes(2))
    expect(updateThumbnailDensity).toHaveBeenNthCalledWith(2, 'maximum')

    await act(async () => maximumSave.resolve(settings('maximum')))
    expect(screen.getByLabelText('density')).toHaveTextContent('maximum')
  })

  it('rolls the latest failed choice back to the last confirmed value', async () => {
    const extraLargeSave = deferred<ViewerSettings>()
    const maximumSave = deferred<ViewerSettings>()
    const updateThumbnailDensity = vi
      .fn<(density: ThumbnailDensity) => Promise<ViewerSettings>>()
      .mockImplementationOnce(() => extraLargeSave.promise)
      .mockImplementationOnce(() => maximumSave.promise)
    renderProvider({
      getViewerSettings: vi.fn().mockResolvedValue(settings('standard')),
      updateThumbnailDensity,
    })

    fireEvent.click(screen.getByRole('button', { name: 'extra_large' }))
    fireEvent.click(screen.getByRole('button', { name: 'maximum' }))
    await waitFor(() => expect(updateThumbnailDensity).toHaveBeenCalledTimes(1))
    await act(async () => extraLargeSave.resolve(settings('extra_large')))
    await waitFor(() => expect(updateThumbnailDensity).toHaveBeenCalledTimes(2))
    await act(async () => maximumSave.reject({ userMessage: '设置未能保存' }))

    expect(screen.getByLabelText('density')).toHaveTextContent('extra_large')
    expect(screen.getByLabelText('height')).toHaveTextContent('204')
    expect(screen.getByRole('alert')).toHaveTextContent('设置未能保存')
  })

  it('clears a previous save error when a new choice begins', async () => {
    const failedSave = deferred<ViewerSettings>()
    const retrySave = deferred<ViewerSettings>()
    const updateThumbnailDensity = vi
      .fn<(density: ThumbnailDensity) => Promise<ViewerSettings>>()
      .mockImplementationOnce(() => failedSave.promise)
      .mockImplementationOnce(() => retrySave.promise)
    renderProvider({
      getViewerSettings: vi.fn().mockResolvedValue(settings('standard')),
      updateThumbnailDensity,
    })

    fireEvent.click(screen.getByRole('button', { name: 'compact' }))
    await waitFor(() => expect(updateThumbnailDensity).toHaveBeenCalledTimes(1))
    await act(async () => failedSave.reject({ userMessage: '设置未能保存' }))
    expect(screen.getByRole('alert')).toHaveTextContent('设置未能保存')

    fireEvent.click(screen.getByRole('button', { name: 'maximum' }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.getByLabelText('density')).toHaveTextContent('maximum')

    await act(async () => retrySave.resolve(settings('maximum')))
    expect(screen.getByLabelText('density')).toHaveTextContent('maximum')
  })

  it('ignores an initial load that arrives after the first user choice', async () => {
    const loaded = deferred<ViewerSettings>()
    const save = deferred<ViewerSettings>()
    renderProvider({
      getViewerSettings: () => loaded.promise,
      updateThumbnailDensity: () => save.promise,
    })

    fireEvent.click(screen.getByRole('button', { name: 'maximum' }))
    await act(async () => loaded.resolve(settings('compact')))

    expect(screen.getByLabelText('density')).toHaveTextContent('maximum')
  })
})
