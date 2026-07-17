import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ViewerBridge } from '../api/viewer'
import EmptyProject from './EmptyProject'

function bridge(): ViewerBridge {
  return {
    chooseProject: vi.fn().mockResolvedValue('/fixture/project'),
    openProject: vi.fn().mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 1,
      displayName: 'project',
      access: 'read_write',
    }),
    closeProject: vi.fn().mockResolvedValue(undefined),
    projectSnapshot: vi.fn().mockResolvedValue(null),
    folderTree: vi.fn().mockResolvedValue([]),
    queryFolder: vi.fn().mockResolvedValue({ workspace: 'empty' }),
    requestImage: vi.fn(),
    previewText: vi.fn(),
    openExternalLink: vi.fn(),
    cancelTask: vi.fn().mockResolvedValue(false),
    listenScan: vi.fn().mockResolvedValue(() => undefined),
    listenProjectClosed: vi.fn().mockResolvedValue(() => undefined),
    listenProjectDrops: vi.fn().mockResolvedValue(() => undefined),
  }
}

describe('EmptyProject', () => {
  it('opens the directory selected by the native chooser', async () => {
    const viewer = bridge()
    render(<EmptyProject bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    await waitFor(() => expect(viewer.openProject).toHaveBeenCalledWith('/fixture/project'))
  })

  it('rejects a dropped file before opening a project', async () => {
    const viewer = bridge()
    render(<EmptyProject bridge={viewer} />)
    const item = {
      webkitGetAsEntry: () => ({ isDirectory: false }),
    }

    fireEvent.drop(screen.getByTestId('project-drop-zone'), {
      dataTransfer: { items: [item], files: [{ name: 'not-a-directory.jpg' }] },
    })

    expect(await screen.findByRole('alert')).toHaveTextContent('请选择项目文件夹')
    expect(viewer.openProject).not.toHaveBeenCalled()
  })
})
