import { useEffect, useMemo, useRef, useState } from 'react'
import ModalSheet from './ModalSheet'
import ViewerButton from './ui/ViewerButton'
import ViewerChoiceChip from './ui/ViewerChoiceChip'
import ViewerField from './ui/ViewerField'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'

interface RenameDialogProps {
  currentName: string
  busy: boolean
  onConfirm: (proposedName: string, editExtension: boolean) => void
  onCancel: () => void
}

export default function RenameDialog({
  currentName,
  busy,
  onConfirm,
  onCancel,
}: RenameDialogProps) {
  const [name, setName] = useState(currentName)
  const [editExtension, setEditExtension] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const validation = useMemo(
    () => validateName(name, currentName, editExtension),
    [currentName, editExtension, name],
  )

  useEffect(() => {
    inputRef.current?.setSelectionRange(0, stemOf(currentName).length)
  }, [currentName])

  return (
    <ModalSheet
      title="重命名文件"
      onCancel={onCancel}
      initialFocusRef={inputRef}
      footer={
        <>
          <ViewerButton tone="secondary" disabled={busy} onClick={onCancel}>
            取消
          </ViewerButton>
          <ViewerButton
            form="rename-file-form"
            type="submit"
            tone="primary"
            loading={busy}
            disabled={validation !== null}
            title={validation ?? (busy ? '正在提交重命名' : '重命名文件')}
          >
            重命名
          </ViewerButton>
        </>
      }
    >
      <form
        id="rename-file-form"
        onSubmit={(event) => {
          event.preventDefault()
          if (validation !== null || busy) return
          onConfirm(editExtension ? name : stemOf(name), editExtension)
        }}
      >
        <ViewerField label="新文件名">
          <input
            ref={inputRef}
            value={name}
            aria-invalid={validation !== null}
            aria-describedby={validation ? 'rename-error' : undefined}
            onChange={(event) => setName(event.currentTarget.value)}
          />
        </ViewerField>
        {validation && (
          <div id="rename-error">
            <ViewerLocalFeedback tone="danger" title="无法使用这个文件名">
              {validation}
            </ViewerLocalFeedback>
          </div>
        )}
        <ViewerChoiceChip checked={editExtension} onCheckedChange={setEditExtension}>
          允许修改扩展名
        </ViewerChoiceChip>
        {busy && (
          <ViewerLocalFeedback tone="info" title="正在提交重命名">
            请等待当前文件操作完成。
          </ViewerLocalFeedback>
        )}
      </form>
    </ModalSheet>
  )
}

function validateName(value: string, currentName: string, editExtension: boolean): string | null {
  if (value.length === 0) return '文件名不能为空。'
  if (value === '.' || value === '..') return '该文件名不可使用。'
  if (value.includes('/') || value.includes('\0')) return '文件名不能包含斜杠或空字符。'
  if (value === '.viewer' || value.startsWith('.viewer-')) return '该名称由 Viewer 保留。'
  if (value === currentName) return '请输入不同的文件名。'
  if (!editExtension && extensionOf(value) !== extensionOf(currentName)) {
    return '如需修改扩展名，请先勾选允许修改扩展名。'
  }
  return null
}

function extensionOf(name: string): string {
  const index = name.lastIndexOf('.')
  return index <= 0 ? '' : name.slice(index + 1)
}

function stemOf(name: string): string {
  const index = name.lastIndexOf('.')
  return index <= 0 ? name : name.slice(0, index)
}
