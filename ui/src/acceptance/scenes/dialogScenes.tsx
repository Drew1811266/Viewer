import { type ReactNode, useCallback, useEffect } from 'react'
import type {
  BatchImageExecution,
  BatchImagePlanRow,
  BatchImageSettingsRequest,
  FileCommandPreflight,
  RenamePreview,
  RenameRules,
  SpeechPreferences,
} from '../../api/types'
import BatchImageDialog from '../../components/BatchImageDialog'
import BatchRenameDialog from '../../components/BatchRenameDialog'
import CloseOperationDialog from '../../components/CloseOperationDialog'
import DestinationDialog from '../../components/DestinationDialog'
import RenameDialog from '../../components/RenameDialog'
import SettingsDialog from '../../components/SettingsDialog'
import TrashConfirmation from '../../components/TrashConfirmation'
import type { SpeechServicePort } from '../../speech/SpeechServiceContext'
import { SpeechServiceProvider } from '../../speech/SpeechServiceContext'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import {
  ACCEPTANCE_FILES,
  ACCEPTANCE_FOLDER_TREE,
  acceptanceVideoCacheBridge,
} from '../acceptanceFixtures'
import AcceptanceProductScene, { setAcceptanceInputValue } from './sceneHarness'

const noOp = () => undefined
const SELECTED_IDS = ACCEPTANCE_FILES.slice(0, 3).map(({ entityId }) => entityId)
const DESTINATION_ID = 'acceptance-folder-destination'
const SPEECH_PREFERENCES: SpeechPreferences = {
  inputMode: 'click',
  locale: 'simplified_chinese',
  autoPunctuation: true,
}
const ACCEPTANCE_SPEECH_STATUS = {
  available: true,
  speechAuthorized: true,
  microphoneAuthorized: true,
  onDevice: true,
  localeSupported: true,
  locale: 'zh-CN',
}

export const DIALOG_SCENES: AcceptanceSceneRegistry = {
  'DIA-01': () => (
    <DialogBackdrop ready={settingsDialogReady}>
      <SettingsDialog
        bridge={acceptanceVideoCacheBridge()}
        density="standard"
        magnifier={{ shape: 'circle', magnification: 2, area: 'small' }}
        error={null}
        onDensityChange={noOp}
        onMagnifierShapeChange={noOp}
        onMagnifierMagnificationChange={noOp}
        onMagnifierAreaChange={noOp}
        onClose={noOp}
      />
    </DialogBackdrop>
  ),
  'DIA-02': () => <SingleRenameScene />,
  'DIA-03': () => <BatchRenameScene />,
  'DIA-04': () => (
    <DialogBackdrop ready={() => document.querySelector('[aria-label="目标检查结果"]') !== null}>
      <DestinationDialog
        mode="copy"
        entityIds={[SELECTED_IDS[0] ?? 'acceptance-image-01']}
        folders={ACCEPTANCE_FOLDER_TREE}
        busy={false}
        initialDestinationId={DESTINATION_ID}
        initialPreflight={READY_PREFLIGHT}
        requestPreflight={async () => READY_PREFLIGHT}
        onConfirm={noOp}
        onCancel={noOp}
      />
    </DialogBackdrop>
  ),
  'DIA-05': () => (
    <DialogBackdrop ready={() => document.querySelector('[data-state="conflict"]') !== null}>
      <DestinationDialog
        mode="copy"
        entityIds={SELECTED_IDS.slice(0, 2)}
        folders={ACCEPTANCE_FOLDER_TREE}
        busy={false}
        initialDestinationId={DESTINATION_ID}
        initialPreflight={CONFLICT_PREFLIGHT}
        requestPreflight={async () => CONFLICT_PREFLIGHT}
        onConfirm={noOp}
        onCancel={noOp}
      />
    </DialogBackdrop>
  ),
  'DIA-06': () => (
    <DialogBackdrop ready={() => dialogIsNamed('将文件移到废纸篓？')}>
      <TrashConfirmation count={3} busy={false} onConfirm={noOp} onCancel={noOp} />
    </DialogBackdrop>
  ),
  'DIA-07': () => (
    <DialogBackdrop ready={() => dialogIsNamed('文件操作尚未完成')}>
      <CloseOperationDialog busy onWait={noOp} onCancelPending={noOp} onStay={noOp} />
    </DialogBackdrop>
  ),
  'DIA-08': () => <BatchImageScene />,
  'DIA-09': () => (
    <DialogBackdrop ready={settingsSpeechReady}>
      <SpeechServiceProvider port={acceptanceSpeechPort()} preferences={SPEECH_PREFERENCES}>
        <SettingsDialog
          bridge={acceptanceVideoCacheBridge()}
          density="standard"
          magnifier={{ shape: 'circle', magnification: 2, area: 'small' }}
          error={null}
          speech={SPEECH_PREFERENCES}
          onDensityChange={noOp}
          onMagnifierShapeChange={noOp}
          onMagnifierMagnificationChange={noOp}
          onMagnifierAreaChange={noOp}
          onSpeechInputModeChange={noOp}
          onSpeechLocaleChange={noOp}
          onSpeechAutoPunctuationChange={noOp}
          onClose={noOp}
        />
      </SpeechServiceProvider>
    </DialogBackdrop>
  ),
}

function acceptanceSpeechPort(): SpeechServicePort {
  return {
    async speechStatus() {
      return ACCEPTANCE_SPEECH_STATUS
    },
    async requestSpeechPermissions() {
      return { speechAuthorized: true, microphoneAuthorized: true }
    },
    async startSpeechCapture() {
      return ACCEPTANCE_SPEECH_STATUS
    },
    async stopSpeechCapture() {
      return { text: '袖口再收紧一些，保留褶皱。', elapsedMs: 2400, onDevice: true }
    },
    cancelSpeechCapture() {},
    async openSpeechSettings() {},
  }
}

function DialogBackdrop({ children, ready }: { children: ReactNode; ready(): boolean }) {
  const stableReady = useCallback(ready, [ready])
  return <AcceptanceProductScene ready={stableReady}>{children}</AcceptanceProductScene>
}

function SingleRenameScene() {
  useEffect(() => {
    const input = document.querySelector<HTMLInputElement>('#rename-file-form input')
    if (input !== null && input.value !== 'A01-正面-01.jpg') {
      setAcceptanceInputValue(input, 'A01-正面-01.jpg')
    }
  }, [])
  const ready = useCallback(
    () =>
      dialogIsNamed('重命名文件') &&
      document.querySelector<HTMLInputElement>('#rename-file-form input')?.value ===
        'A01-正面-01.jpg' &&
      document.querySelector('#rename-error') === null,
    [],
  )
  return (
    <DialogBackdrop ready={ready}>
      <RenameDialog currentName="商品-01.jpg" busy={false} onConfirm={noOp} onCancel={noOp} />
    </DialogBackdrop>
  )
}

function BatchRenameScene() {
  useEffect(() => {
    const act = () => {
      const prefix = [...document.querySelectorAll<HTMLInputElement>('input')].find((input) =>
        input.closest('.viewer-field')?.textContent?.includes('前缀'),
      )
      if (prefix !== undefined && prefix.value !== '精选-') {
        setAcceptanceInputValue(prefix, '精选-')
        queueMicrotask(act)
        return
      }
      const sequence = [...document.querySelectorAll<HTMLLabelElement>('label')]
        .find((label) => label.textContent?.includes('添加序号'))
        ?.querySelector<HTMLInputElement>('input')
      if (sequence !== undefined && sequence !== null && !sequence.checked) {
        sequence.click()
        queueMicrotask(act)
        return
      }
      const update = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === '更新预览',
      )
      if (
        update !== undefined &&
        document.querySelector('[aria-label="批量重命名完整预览"]') === null
      ) {
        update.click()
      }
    }
    const observer = new MutationObserver(act)
    observer.observe(document.body, { attributes: true, childList: true, subtree: true })
    act()
    return () => observer.disconnect()
  }, [])
  const ready = useCallback(
    () =>
      [...document.querySelectorAll<HTMLInputElement>('.viewer-field input')].some(
        (input) =>
          input.closest('.viewer-field')?.textContent?.includes('前缀') && input.value === '精选-',
      ) && document.querySelector('[aria-label="批量重命名完整预览"]') !== null,
    [],
  )
  return (
    <AcceptanceProductScene ready={ready}>
      <BatchRenameDialog
        entityIds={SELECTED_IDS}
        busy={false}
        requestPreview={requestRenamePreview}
        onConfirm={noOp}
        onCancel={noOp}
      />
    </AcceptanceProductScene>
  )
}

async function requestRenamePreview(
  entityIds: string[],
  rules: RenameRules,
): Promise<RenamePreview> {
  return {
    executable: true,
    rows: entityIds.map((entityId, index) => {
      const source = ACCEPTANCE_FILES[index]
      const number = rules.sequence
        ? String(rules.sequence.start + index).padStart(rules.sequence.digits, '0')
        : ''
      const proposedName = `${rules.prefix}${number}${source?.name ?? `商品-${index + 1}.jpg`}`
      return {
        entityId,
        sourceRelativePath: source?.relativePath ?? proposedName,
        destinationRelativePath: `衣服/A01/${proposedName}`,
        proposedName,
        errors: [],
      }
    }),
  }
}

const READY_PREFLIGHT: FileCommandPreflight = {
  executable: true,
  rows: [
    {
      entityId: SELECTED_IDS[0] ?? 'acceptance-image-01',
      relativePath: '目标/Destination/商品-01.jpg',
      state: 'ready',
    },
  ],
}

const CONFLICT_PREFLIGHT: FileCommandPreflight = {
  executable: true,
  rows: SELECTED_IDS.slice(0, 2).map((entityId, index) => ({
    entityId,
    relativePath: `目标/Destination/商品-0${index + 1}.jpg`,
    state: 'conflict' as const,
    code: 'destination_occupied' as const,
  })),
}

function BatchImageScene() {
  const ready = useCallback(
    () =>
      dialogIsNamed('批量处理图片') &&
      document.querySelector('[data-testid="batch-image-plan"]') !== null,
    [],
  )
  return (
    <AcceptanceProductScene ready={ready}>
      <BatchImageDialog
        entityIds={SELECTED_IDS}
        busy={false}
        previewBatchImage={requestBatchImagePreview}
        executeBatchImage={requestBatchImageExecution}
        onCancel={noOp}
      />
    </AcceptanceProductScene>
  )
}

async function requestBatchImagePreview(
  entityIds: readonly string[],
  settings: BatchImageSettingsRequest,
): Promise<BatchImagePlanRow[]> {
  return entityIds.map((entityId, index) => {
    const source = ACCEPTANCE_FILES[index]
    const suffix = settings.output.kind === 'sibling' ? settings.output.suffix : ''
    return {
      entityId,
      relativePath: source?.relativePath ?? `衣服/A01/商品-${index + 1}.jpg`,
      sourceSizeBytes: source?.size ?? 32_768,
      targetWidth: 1024,
      targetHeight: 768,
      estimatedSizeBytes: 16_384,
      disposition: {
        kind: 'convert',
        destination: `衣服/A01/商品-0${index + 1}${suffix}.jpg`,
      },
    }
  })
}

async function requestBatchImageExecution(): Promise<BatchImageExecution> {
  return { batchId: 'acceptance-batch-image', outcomes: [] }
}

function dialogIsNamed(title: string): boolean {
  const dialog = document.querySelector('[role="dialog"]')
  if (dialog === null) return false
  return [...dialog.querySelectorAll('h2')].some((heading) => heading.textContent?.trim() === title)
}

function settingsSpeechReady(): boolean {
  const dialog = document.querySelector<HTMLElement>('[role="dialog"]')
  if (dialog === null || !dialogIsNamed('软件设置')) return false
  const navigation = dialog.querySelector<HTMLElement>('[aria-label="设置分类"]')
  const speechNav = [...(navigation?.querySelectorAll<HTMLButtonElement>('button') ?? [])].find(
    (button) => button.textContent?.trim() === '语音与转写',
  )
  if (speechNav === undefined) return false
  if (speechNav.getAttribute('aria-pressed') !== 'true') {
    if (speechNav.dataset.acceptanceAction !== 'select-speech') {
      speechNav.dataset.acceptanceAction = 'select-speech'
      speechNav.click()
    }
    return false
  }
  return (
    dialog.querySelector('.settings-speech-privacy') !== null &&
    dialog.querySelector('.settings-status-chip')?.textContent?.trim() === '已就绪' &&
    dialog.querySelector('[aria-label="识别语言"]') !== null &&
    dialog.querySelector('[aria-label="自动标点"]') !== null
  )
}

function settingsDialogReady(): boolean {
  const dialog = document.querySelector<HTMLElement>('[role="dialog"]')
  if (dialog === null || !dialogIsNamed('软件设置')) return false
  const slider = dialog.querySelector<HTMLInputElement>('[aria-label="缩略图大小"]')
  const checkedLabels = [...dialog.querySelectorAll<HTMLInputElement>('input[type="radio"]')]
    .filter((input) => input.checked)
    .map((input) => input.closest('label')?.textContent?.trim())
  const magnificationLabels = [
    ...dialog.querySelectorAll<HTMLInputElement>('input[name="magnifier-magnification"]'),
  ].map((input) => input.closest('label')?.textContent?.trim())
  return (
    slider?.value === '2' &&
    magnificationLabels.join('|') === '2 倍|3 倍|4 倍' &&
    checkedLabels.includes('圆形') &&
    checkedLabels.includes('2 倍') &&
    checkedLabels.includes('小')
  )
}
