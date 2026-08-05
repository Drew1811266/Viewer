import { type ReactNode, useCallback, useEffect } from 'react'
import type { OperationProgressEvent, OperationResultPage, ProjectSnapshot } from '../../api/types'
import DestinationDialog from '../../components/DestinationDialog'
import GlobalNoticeStack from '../../components/GlobalNoticeStack'
import OperationResults from '../../components/OperationResults'
import ReadOnlyBanner from '../../components/ReadOnlyBanner'
import SettingsDialog from '../../components/SettingsDialog'
import TaskBar, { type TaskFeedback } from '../../components/TaskBar'
import ViewerButton from '../../components/ui/ViewerButton'
import ViewerLocalFeedback from '../../components/ui/ViewerLocalFeedback'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import type { AcceptanceBridgeOverrides } from '../acceptanceBridge'
import {
  ACCEPTANCE_FILES,
  ACCEPTANCE_FOLDER_TREE,
  ACCEPTANCE_PROJECT_SNAPSHOT,
} from '../acceptanceFixtures'
import AcceptanceProductScene from './sceneHarness'

const noOp = () => undefined

export const FEEDBACK_SCENES: AcceptanceSceneRegistry = {
  'TAS-01': () => <TaskScene task={RUNNING_TASK} />,
  'TAS-02': () => <TaskScene task={SUCCESS_TASK} />,
  'TAS-03': () => <ExpandedFailureTaskScene />,
  'TAS-04': () => <TaskScene task={CANCELLED_TASK} />,
  'TAS-05': () => <TaskScene task={RESULT_TASK} />,
  'RES-01': () => (
    <FeedbackBackdrop ready={() => document.querySelector('[aria-label="文件操作结果"]') !== null}>
      <OperationResults
        batchId="acceptance-operation"
        progress={OPERATION_PROGRESS}
        page={OPERATION_RESULTS}
        onPageChange={noOp}
        onClose={noOp}
      />
    </FeedbackBackdrop>
  ),
  'RES-02': () => (
    <FeedbackBackdrop ready={() => document.querySelector('[aria-label="全局通知"]') !== null}>
      <GlobalNoticeStack
        notices={[
          {
            id: 'acceptance-index-updated',
            title: '索引已更新',
            message: '新增 12 个文件，缩略图正在后台补齐。',
            tone: 'info',
          },
          {
            id: 'acceptance-context-repair',
            title: '文件位置已更新',
            message: '已保留当前选择，并定位到移动后的文件。',
            tone: 'warning',
            action: { label: '查看新位置', onAction: noOp },
          },
        ]}
      />
    </FeedbackBackdrop>
  ),
  'RES-03': () => <LocalFeedbackScene />,
  'RES-04': () => <ReadOnlyScene />,
  'RES-05': () => (
    <FeedbackBackdrop ready={() => document.querySelector('[aria-label="文件操作结果"]') !== null}>
      <GlobalNoticeStack
        notices={[
          {
            id: 'acceptance-recovery',
            title: '已恢复中断的文件操作',
            message: '3 项已经完成，1 项需要检查。',
            tone: 'info',
            action: { label: '查看恢复结果', onAction: noOp },
          },
        ]}
      />
      <OperationResults
        batchId="acceptance-recovery"
        progress={OPERATION_PROGRESS}
        page={OPERATION_RESULTS}
        onPageChange={noOp}
        onClose={noOp}
      />
    </FeedbackBackdrop>
  ),
  'A11Y-01': () => (
    <FeedbackBackdrop
      ready={() => document.activeElement?.getAttribute('name') === 'thumbnail-density'}
      attributes={{ 'data-acceptance-accessibility': 'keyboard' }}
    >
      <SettingsDialog density="standard" error={null} onDensityChange={noOp} onClose={noOp} />
    </FeedbackBackdrop>
  ),
  'A11Y-02': () => <FocusRestorationScene />,
  'A11Y-03': () => (
    <FeedbackBackdrop
      ready={() => document.querySelector('[aria-label="后台任务"]') !== null}
      attributes={{ 'data-acceptance-motion': 'reduced' }}
    >
      <TaskBar tasks={[RUNNING_TASK, RESULT_TASK]} onCancel={noOp} onShowResults={noOp} />
    </FeedbackBackdrop>
  ),
  'A11Y-04': () => (
    <FeedbackBackdrop
      ready={() => document.querySelector('[data-state="conflict"]') !== null}
      attributes={{ 'data-acceptance-contrast': 'forced-colors' }}
    >
      <DestinationDialog
        mode="copy"
        entityIds={ACCEPTANCE_FILES.slice(0, 2).map(({ entityId }) => entityId)}
        folders={ACCEPTANCE_FOLDER_TREE}
        busy={false}
        initialDestinationId="acceptance-folder-destination"
        initialPreflight={{
          executable: true,
          rows: ACCEPTANCE_FILES.slice(0, 2).map((file) => ({
            entityId: file.entityId,
            relativePath: `目标/Destination/${file.name}`,
            state: 'conflict' as const,
            code: 'destination_occupied' as const,
          })),
        }}
        requestPreflight={async () => null}
        onConfirm={noOp}
        onCancel={noOp}
      />
    </FeedbackBackdrop>
  ),
  'A11Y-05': () => (
    <FeedbackBackdrop
      ready={() =>
        [...document.querySelectorAll('[role="dialog"] h2')].some(
          (heading) => heading.textContent?.trim() === '软件设置',
        ) && document.querySelector('[aria-label="后台任务"]') !== null
      }
      attributes={{ 'data-acceptance-zoom': '200' }}
    >
      <TaskBar task={RESULT_TASK} onShowResults={noOp} />
      <SettingsDialog density="compact" error={null} onDensityChange={noOp} onClose={noOp} />
    </FeedbackBackdrop>
  ),
}

function FeedbackBackdrop({
  children,
  ready,
  bridgeOverrides,
  attributes,
}: {
  children: ReactNode
  ready(): boolean
  bridgeOverrides?: AcceptanceBridgeOverrides
  attributes?: Record<string, string>
}) {
  const stableReady = useCallback(ready, [ready])
  return (
    <AcceptanceProductScene
      ready={stableReady}
      bridgeOverrides={bridgeOverrides}
      attributes={attributes}
    >
      {children}
    </AcceptanceProductScene>
  )
}

function TaskScene({ task }: { task: TaskFeedback }) {
  return (
    <FeedbackBackdrop ready={() => document.querySelector('[aria-label="后台任务"]') !== null}>
      <TaskBar
        task={task}
        onCancel={noOp}
        onDismiss={noOp}
        onShowResults={noOp}
        successDismissMs={10_000}
      />
    </FeedbackBackdrop>
  )
}

function LocalFeedbackScene() {
  useEffect(() => {
    const openContentFolder = () => navigateToLocalFeedbackFolder(document)
    const observer = new MutationObserver(openContentFolder)
    observer.observe(document.body, { attributes: true, childList: true, subtree: true })
    openContentFolder()
    return () => observer.disconnect()
  }, [])
  const ready = useCallback(
    () =>
      document.querySelector('.image-cell') !== null &&
      document.querySelector('[data-acceptance-local-feedback]') !== null,
    [],
  )
  return (
    <FeedbackBackdrop ready={ready}>
      <section className="acceptance-local-feedback-stage" data-acceptance-local-feedback>
        <ViewerLocalFeedback
          tone="danger"
          title="无法显示这张图片"
          action={<ViewerButton onClick={noOp}>重新载入</ViewerButton>}
        >
          文件已被移动或当前没有读取权限。
        </ViewerLocalFeedback>
      </section>
    </FeedbackBackdrop>
  )
}

export function navigateToLocalFeedbackFolder(root: Document | Element) {
  if (root.querySelector('.image-cell') !== null) return 'ready'
  const folder = root.querySelector<HTMLElement>('[role="treeitem"][aria-label="衣服/A01"]')
  if (folder !== null && folder.dataset.acceptanceAction !== 'open-local-feedback-folder') {
    folder.dataset.acceptanceAction = 'open-local-feedback-folder'
    folder.click()
    return 'opening'
  }
  const disclosure = root.querySelector<HTMLElement>('[aria-label="展开 衣服"]')
  if (
    disclosure !== null &&
    disclosure.dataset.acceptanceAction !== 'expand-local-feedback-parent'
  ) {
    disclosure.dataset.acceptanceAction = 'expand-local-feedback-parent'
    disclosure.click()
    return 'expanding'
  }
  return 'waiting'
}

function ExpandedFailureTaskScene() {
  useEffect(() => {
    const expand = () => {
      if (document.querySelector('.task-details li') !== null) return
      const button = document.querySelector<HTMLElement>('[aria-label="展开任务详情"]')
      if (button !== null && button.dataset.acceptanceAction !== 'expand-task') {
        button.dataset.acceptanceAction = 'expand-task'
        button.click()
      }
    }
    const observer = new MutationObserver(expand)
    observer.observe(document.body, { attributes: true, childList: true, subtree: true })
    expand()
    return () => observer.disconnect()
  }, [])
  const ready = useCallback(() => document.querySelector('.task-details li') !== null, [])
  return (
    <AcceptanceProductScene ready={ready}>
      <TaskBar task={FAILED_TASK} onDismiss={noOp} successDismissMs={10_000} />
    </AcceptanceProductScene>
  )
}

function ReadOnlyScene() {
  const readOnlySnapshot: ProjectSnapshot = {
    ...ACCEPTANCE_PROJECT_SNAPSHOT,
    access: 'read_only',
  }
  const bridgeOverrides: AcceptanceBridgeOverrides = {
    async openProject() {
      return readOnlySnapshot
    },
  }
  return (
    <FeedbackBackdrop
      ready={() => document.querySelector('[aria-label="只读模式"]') !== null}
      bridgeOverrides={bridgeOverrides}
    >
      <ReadOnlyBanner busy={false} onOpenSettings={noOp} onReselect={noOp} />
    </FeedbackBackdrop>
  )
}

function FocusRestorationScene() {
  useEffect(() => {
    const focus = () => {
      const more = document.querySelector<HTMLElement>('[aria-label="更多"]')
      if (more !== null) more.focus()
    }
    const observer = new MutationObserver(focus)
    observer.observe(document.body, { childList: true, subtree: true })
    focus()
    return () => observer.disconnect()
  }, [])
  const ready = useCallback(() => document.activeElement?.getAttribute('aria-label') === '更多', [])
  return (
    <AcceptanceProductScene
      ready={ready}
      attributes={{ 'data-acceptance-accessibility': 'focus-restored' }}
    >
      {null}
    </AcceptanceProductScene>
  )
}

const RUNNING_TASK: TaskFeedback = {
  id: 'acceptance-running',
  label: '正在生成缩略图',
  status: 'running',
  requested: 30,
  completed: 18,
  failed: 0,
  cancellable: true,
  failures: [],
}

const SUCCESS_TASK: TaskFeedback = {
  id: 'acceptance-success',
  label: '复制文件',
  status: 'complete',
  requested: 3,
  completed: 3,
  failed: 0,
  cancellable: false,
  failures: [],
}

const FAILED_TASK: TaskFeedback = {
  id: 'acceptance-failed',
  label: '整理文件',
  status: 'failed',
  requested: 4,
  completed: 3,
  failed: 1,
  cancellable: false,
  failures: [{ item: '衣服/A01/商品-04.jpg', code: 'permission_denied' }],
}

const CANCELLED_TASK: TaskFeedback = {
  id: 'acceptance-cancelled',
  label: '复制文件',
  status: 'cancelled',
  requested: 30,
  completed: 8,
  failed: 0,
  cancelled: 22,
  cancellable: false,
  failures: [],
}

const RESULT_TASK: TaskFeedback = {
  id: 'acceptance-results',
  label: '整理文件',
  status: 'complete',
  requested: 4,
  completed: 3,
  failed: 1,
  cancellable: false,
  failures: [{ item: '衣服/A01/商品-04.jpg', code: 'permission_denied' }],
  hasResults: true,
}

const OPERATION_PROGRESS: OperationProgressEvent = {
  sessionId: 'acceptance-session',
  generation: 1,
  batchId: 'acceptance-operation',
  lifecycle: 'completed',
  requested: 4,
  completed: 2,
  failed: 1,
  skipped: 1,
  cancelled: 0,
  activeEntityId: null,
}

const OPERATION_RESULTS: OperationResultPage = {
  total: 4,
  offset: 0,
  items: [
    {
      entityId: 'acceptance-image-01',
      relativePath: '衣服/A01/商品-01.jpg',
      status: 'completed',
      code: 'copied',
    },
    {
      entityId: 'acceptance-image-02',
      relativePath: '衣服/A01/商品-02.jpg',
      status: 'completed',
      code: 'copied',
    },
    {
      entityId: 'acceptance-image-03',
      relativePath: '衣服/A01/商品-03.jpg',
      status: 'failed',
      code: 'permission_denied',
    },
    {
      entityId: 'acceptance-image-04',
      relativePath: '衣服/A01/商品-04.jpg',
      status: 'skipped',
      code: 'conflict_skipped',
    },
  ],
}
