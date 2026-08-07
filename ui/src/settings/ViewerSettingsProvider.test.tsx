import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type {
  MagnifierPreferences,
  ThumbnailDensity,
  ViewerSettings,
  ViewerSettingsUpdate,
} from '../api/types'
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

const DEFAULT_MAGNIFIER: MagnifierPreferences = {
  shape: 'circle',
  magnification: 4,
  area: 'small',
}

function settings(
  thumbnailDensity: ThumbnailDensity,
  magnifier: MagnifierPreferences = DEFAULT_MAGNIFIER,
): ViewerSettings {
  return { schemaVersion: 2, thumbnailDensity, magnifier }
}

function Consumer() {
  const current = useViewerSettings()
  return (
    <>
      <output aria-label="density">{current.thumbnailDensity}</output>
      <output aria-label="height">{current.thumbnailHeight}</output>
      <output aria-label="shape">{current.magnifier.shape}</output>
      <output aria-label="magnification">{current.magnifier.magnification}</output>
      <output aria-label="area">{current.magnifier.area}</output>
      {current.settingsError && <p role="alert">{current.settingsError}</p>}
      <button type="button" onClick={() => current.setThumbnailDensity('maximum')}>
        maximum
      </button>
      <button type="button" onClick={() => current.setMagnifierShape('rounded_rectangle')}>
        rectangle
      </button>
      <button type="button" onClick={() => current.setMagnifierMagnification(6)}>
        six
      </button>
      <button type="button" onClick={() => current.setMagnifierArea('large')}>
        large-area
      </button>
    </>
  )
}

function renderProvider(
  bridge: {
    getViewerSettings: () => Promise<ViewerSettings>
    updateViewerSettings: (settings: ViewerSettingsUpdate) => Promise<ViewerSettings>
  },
  children: ReactNode = <Consumer />,
) {
  return render(<ViewerSettingsProvider bridge={bridge}>{children}</ViewerSettingsProvider>)
}

describe('ViewerSettingsProvider', () => {
  it('starts with complete defaults and adopts every loaded setting', async () => {
    const loaded = deferred<ViewerSettings>()
    renderProvider({ getViewerSettings: () => loaded.promise, updateViewerSettings: vi.fn() })

    expect(screen.getByLabelText('density')).toHaveTextContent('standard')
    expect(screen.getByLabelText('height')).toHaveTextContent('132')
    expect(screen.getByLabelText('shape')).toHaveTextContent('circle')
    expect(screen.getByLabelText('magnification')).toHaveTextContent('4')
    expect(screen.getByLabelText('area')).toHaveTextContent('small')

    loaded.resolve(
      settings('compact', {
        shape: 'rounded_rectangle',
        magnification: 5,
        area: 'medium',
      }),
    )
    await waitFor(() => expect(screen.getByLabelText('density')).toHaveTextContent('compact'))
    expect(screen.getByLabelText('height')).toHaveTextContent('96')
    expect(screen.getByLabelText('shape')).toHaveTextContent('rounded_rectangle')
    expect(screen.getByLabelText('magnification')).toHaveTextContent('5')
    expect(screen.getByLabelText('area')).toHaveTextContent('medium')
  })

  it('keeps complete defaults without a blocking error when initial loading fails', async () => {
    const loaded = deferred<ViewerSettings>()
    renderProvider({ getViewerSettings: () => loaded.promise, updateViewerSettings: vi.fn() })

    await act(async () => loaded.reject(new Error('offline')))
    expect(screen.getByLabelText('density')).toHaveTextContent('standard')
    expect(screen.getByLabelText('shape')).toHaveTextContent('circle')
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('publishes a complete optimistic shape choice before save resolves', async () => {
    const save = deferred<ViewerSettings>()
    const updateViewerSettings = vi.fn(() => save.promise)
    renderProvider({
      getViewerSettings: vi.fn().mockResolvedValue(settings('standard')),
      updateViewerSettings,
    })

    fireEvent.click(screen.getByRole('button', { name: 'rectangle' }))

    expect(screen.getByLabelText('shape')).toHaveTextContent('rounded_rectangle')
    await waitFor(() => expect(updateViewerSettings).toHaveBeenCalledOnce())
    expect(updateViewerSettings).toHaveBeenCalledWith({
      thumbnailDensity: 'standard',
      magnifier: { shape: 'rounded_rectangle', magnification: 4, area: 'small' },
    })
  })

  it('serializes cross-field writes without overwriting the prior optimistic field', async () => {
    const shapeSave = deferred<ViewerSettings>()
    const magnificationSave = deferred<ViewerSettings>()
    const updateViewerSettings = vi
      .fn<(value: ViewerSettingsUpdate) => Promise<ViewerSettings>>()
      .mockImplementationOnce(() => shapeSave.promise)
      .mockImplementationOnce(() => magnificationSave.promise)
    renderProvider({
      getViewerSettings: vi.fn().mockResolvedValue(settings('standard')),
      updateViewerSettings,
    })

    fireEvent.click(screen.getByRole('button', { name: 'rectangle' }))
    fireEvent.click(screen.getByRole('button', { name: 'six' }))
    expect(screen.getByLabelText('shape')).toHaveTextContent('rounded_rectangle')
    expect(screen.getByLabelText('magnification')).toHaveTextContent('6')
    await waitFor(() => expect(updateViewerSettings).toHaveBeenCalledTimes(1))

    await act(async () =>
      shapeSave.resolve(
        settings('standard', {
          shape: 'rounded_rectangle',
          magnification: 4,
          area: 'small',
        }),
      ),
    )
    await waitFor(() => expect(updateViewerSettings).toHaveBeenCalledTimes(2))
    expect(updateViewerSettings).toHaveBeenNthCalledWith(2, {
      thumbnailDensity: 'standard',
      magnifier: { shape: 'rounded_rectangle', magnification: 6, area: 'small' },
    })

    await act(async () =>
      magnificationSave.resolve(
        settings('standard', {
          shape: 'rounded_rectangle',
          magnification: 6,
          area: 'small',
        }),
      ),
    )
    expect(screen.getByLabelText('magnification')).toHaveTextContent('6')
  })

  it('rolls every field back to the last confirmed complete snapshot', async () => {
    const shapeSave = deferred<ViewerSettings>()
    const areaSave = deferred<ViewerSettings>()
    const updateViewerSettings = vi
      .fn<(value: ViewerSettingsUpdate) => Promise<ViewerSettings>>()
      .mockImplementationOnce(() => shapeSave.promise)
      .mockImplementationOnce(() => areaSave.promise)
    renderProvider({
      getViewerSettings: vi.fn().mockResolvedValue(settings('standard')),
      updateViewerSettings,
    })

    fireEvent.click(screen.getByRole('button', { name: 'rectangle' }))
    fireEvent.click(screen.getByRole('button', { name: 'large-area' }))
    await waitFor(() => expect(updateViewerSettings).toHaveBeenCalledTimes(1))
    await act(async () =>
      shapeSave.resolve(
        settings('standard', {
          shape: 'rounded_rectangle',
          magnification: 4,
          area: 'small',
        }),
      ),
    )
    await waitFor(() => expect(updateViewerSettings).toHaveBeenCalledTimes(2))
    await act(async () => areaSave.reject({ userMessage: '设置未能保存' }))

    expect(screen.getByLabelText('shape')).toHaveTextContent('rounded_rectangle')
    expect(screen.getByLabelText('magnification')).toHaveTextContent('4')
    expect(screen.getByLabelText('area')).toHaveTextContent('small')
    expect(screen.getByRole('alert')).toHaveTextContent('设置未能保存')
  })

  it('clears a save error when a new complete choice begins', async () => {
    const failedSave = deferred<ViewerSettings>()
    const retrySave = deferred<ViewerSettings>()
    const updateViewerSettings = vi
      .fn<(value: ViewerSettingsUpdate) => Promise<ViewerSettings>>()
      .mockImplementationOnce(() => failedSave.promise)
      .mockImplementationOnce(() => retrySave.promise)
    renderProvider({
      getViewerSettings: vi.fn().mockResolvedValue(settings('standard')),
      updateViewerSettings,
    })

    fireEvent.click(screen.getByRole('button', { name: 'rectangle' }))
    await waitFor(() => expect(updateViewerSettings).toHaveBeenCalledTimes(1))
    await act(async () => failedSave.reject({ userMessage: '设置未能保存' }))
    expect(screen.getByRole('alert')).toHaveTextContent('设置未能保存')

    fireEvent.click(screen.getByRole('button', { name: 'maximum' }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.getByLabelText('density')).toHaveTextContent('maximum')
  })

  it('ignores an initial load that arrives after the first user choice', async () => {
    const loaded = deferred<ViewerSettings>()
    const save = deferred<ViewerSettings>()
    renderProvider({
      getViewerSettings: () => loaded.promise,
      updateViewerSettings: () => save.promise,
    })

    fireEvent.click(screen.getByRole('button', { name: 'six' }))
    await act(async () =>
      loaded.resolve(
        settings('compact', { shape: 'rounded_rectangle', magnification: 3, area: 'large' }),
      ),
    )

    expect(screen.getByLabelText('density')).toHaveTextContent('standard')
    expect(screen.getByLabelText('shape')).toHaveTextContent('circle')
    expect(screen.getByLabelText('magnification')).toHaveTextContent('6')
    expect(screen.getByLabelText('area')).toHaveTextContent('small')
  })
})
