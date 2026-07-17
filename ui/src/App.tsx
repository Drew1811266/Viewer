import { useCallback, useEffect, useMemo, useState } from 'react'
import type { ViewerBridge } from './api/viewer'
import { tauriViewerBridge } from './api/viewer'
import EmptyProject from './components/EmptyProject'
import ContentBrowser from './components/ContentBrowser'
import FolderOverview from './components/FolderOverview'
import FolderTree from './components/FolderTree'
import TaskBar from './components/TaskBar'
import type { TaskFeedback } from './components/TaskBar'
import { useViewerController } from './state/useViewerController'
import type { BrowserFile } from './api/types'

interface AppProps {
  bridge?: ViewerBridge
}

export default function App({ bridge = tauriViewerBridge }: AppProps) {
  const { state, openProject, closeProject, selectFolder, showAllDescendants, cancelTask } =
    useViewerController(bridge)
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useState(260)
  const [thumbnailTask, setThumbnailTask] = useState<TaskFeedback | null>(null)
  const [dismissedTasks, setDismissedTasks] = useState<Set<string>>(() => new Set())
  useEffect(() => {
    setThumbnailTask(null)
    setDismissedTasks(new Set())
  }, [state.project?.sessionId])
  const requestThumbnail = useCallback(
    (file: BrowserFile) =>
      bridge
        .requestImage({
          entityId: file.entityId,
          representation: { kind: 'thumbnail', maxPixels: 320, scaleMilli: 1_000 },
        })
        .then((image) => image.url),
    [bridge],
  )
  const requestContentThumbnail = useCallback(
    (file: BrowserFile, maxPixels: number, scaleMilli: number) =>
      bridge
        .requestImage({
          entityId: file.entityId,
          representation: { kind: 'thumbnail', maxPixels, scaleMilli },
        })
        .then((image) => image.url),
    [bridge],
  )
  const scanTask = useMemo<TaskFeedback | null>(() => {
    if (state.scan === null) return null
    const published = state.scan.publishedFolders + state.scan.publishedFiles
    const failed = state.scan.totals?.failed ?? state.scan.failedItems.length
    const requested = state.scan.totals
      ? state.scan.totals.folders + state.scan.totals.files + failed
      : published + failed + 1
    return {
      id: state.scan.taskId,
      label: '扫描项目',
      status:
        state.scan.phase === 'running'
          ? 'running'
          : state.scan.phase === 'cancelled'
            ? 'cancelled'
            : failed > 0
              ? 'failed'
              : 'complete',
      requested,
      completed: state.scan.totals
        ? state.scan.totals.folders + state.scan.totals.files
        : published,
      failed,
      cancellable: state.scan.phase === 'running',
      failures: state.scan.failedItems.map((failure) => ({
        item: failure.relativePath,
        code: failure.code,
      })),
    }
  }, [state.scan])
  const visibleTasks = useMemo(
    () =>
      [scanTask, thumbnailTask].filter(
        (task): task is TaskFeedback =>
          task !== null && (!dismissedTasks.has(task.id) || task.status === 'running'),
      ),
    [dismissedTasks, scanTask, thumbnailTask],
  )

  if (state.project === null) {
    return (
      <EmptyProject
        bridge={bridge}
        busy={state.status === 'opening'}
        errorMessage={state.errorMessage}
        onOpenProject={openProject}
      />
    )
  }

  return (
    <main className="viewer-shell">
      <header>
        <h1>{state.project.displayName}</h1>
        <button
          type="button"
          disabled={state.status === 'closing'}
          onClick={() => void closeProject()}
        >
          {state.status === 'closing' ? '正在关闭…' : '关闭项目'}
        </button>
      </header>
      {state.project.access === 'read_only' && (
        <p className="read-only-banner" role="status">
          只读项目
        </p>
      )}
      {state.errorMessage && <p role="alert">{state.errorMessage}</p>}
      <div className="viewer-columns">
        <aside
          className="folder-sidebar"
          aria-label="文件夹栏"
          style={{ width: sidebarCollapsed ? 44 : sidebarWidth }}
        >
          <button
            type="button"
            aria-label={sidebarCollapsed ? '展开文件夹栏' : '折叠文件夹栏'}
            onClick={() => setSidebarCollapsed((collapsed) => !collapsed)}
          >
            {sidebarCollapsed ? '›' : '‹'}
          </button>
          {!sidebarCollapsed && (
            <>
              <button
                type="button"
                className="project-root-button"
                aria-pressed={state.selectedFolderId === null}
                onClick={() => void selectFolder(null)}
              >
                项目根目录
              </button>
              <FolderTree
                folders={state.folders}
                selectedId={state.selectedFolderId}
                onSelect={(entityId) => void selectFolder(entityId)}
              />
              <label className="sidebar-resize">
                文件夹栏宽度
                <input
                  type="range"
                  min="200"
                  max="420"
                  value={sidebarWidth}
                  onChange={(event) => setSidebarWidth(Number(event.currentTarget.value))}
                />
              </label>
            </>
          )}
        </aside>
        <section className="workspace" aria-label="项目内容">
          {state.workspace === null && <p>正在读取项目…</p>}
          {state.workspace?.workspace === 'empty' && <p>此文件夹中没有支持的文件。</p>}
          {state.workspace?.workspace === 'category' && (
            <FolderOverview
              folders={state.workspace.folders}
              currentPath={state.selectedFolderPath || state.project.displayName}
              requestThumbnail={requestThumbnail}
              onSelect={(entityId) => void selectFolder(entityId)}
              onShowAll={() => void showAllDescendants()}
            />
          )}
          {state.workspace?.workspace === 'content' && (
            <>
              {state.showingAggregate && <p className="aggregate-label">全部后代文件</p>}
              <ContentBrowser
                workspace={state.workspace}
                requestThumbnail={requestContentThumbnail}
                onThumbnailTaskChange={setThumbnailTask}
              />
            </>
          )}
        </section>
      </div>
      <TaskBar
        tasks={visibleTasks}
        onCancel={(taskId) => void cancelTask(taskId)}
        onDismiss={(taskId) =>
          setDismissedTasks((current) => new Set([...current, taskId]))
        }
      />
    </main>
  )
}
