import { useEffect, useMemo, useRef, useState } from 'react'
import ModalSheet from './ModalSheet'

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
    <ModalSheet title="重命名文件" onCancel={onCancel} initialFocusRef={inputRef}>
      <form
        onSubmit={(event) => {
          event.preventDefault()
          if (validation !== null || busy) return
          onConfirm(editExtension ? name : stemOf(name), editExtension)
        }}
      >
        <label className="form-field">
          <span>新文件名</span>
          <input
            ref={inputRef}
            value={name}
            aria-invalid={validation !== null}
            aria-describedby={validation ? 'rename-error' : undefined}
            onChange={(event) => setName(event.currentTarget.value)}
          />
        </label>
        {validation && (
          <p id="rename-error" className="form-error" role="alert">
            {validation}
          </p>
        )}
        <label className="checkbox-field">
          <input
            type="checkbox"
            checked={editExtension}
            onChange={(event) => setEditExtension(event.currentTarget.checked)}
          />
          允许修改扩展名
        </label>
        <div className="modal-actions">
          <button type="button" onClick={onCancel}>
            取消
          </button>
          <button type="submit" disabled={validation !== null || busy}>
            {busy ? '正在提交…' : '重命名'}
          </button>
        </div>
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
