import type { ReviewState } from '../api/types'

export type RadialLeafAction =
  | 'preview'
  | 'mark.keep'
  | 'mark.pending'
  | 'mark.reject'
  | 'mark.clear'
  | 'mark.favorite'
  | 'organize.rename'
  | 'organize.copy'
  | 'organize.move'
  | 'trash'
  | 'compare'
  | 'info'

export type RadialPrimaryId =
  | 'preview'
  | 'mark'
  | 'organize'
  | 'trash'
  | 'compare'
  | 'info'

export interface RadialMenuContext {
  selectedCount: number
  selectedImageCount: number
  readOnly: boolean
  busy: boolean
  compareContextAvailable: boolean
  commonReview: ReviewState | null | 'mixed'
  commonFavorite: boolean | 'mixed'
}

export interface RadialMenuItem {
  id: RadialPrimaryId | RadialLeafAction
  label: string
  symbol: string
  disabled: boolean
  disabledReason?: string
  tone?: 'normal' | 'destructive'
  checked?: boolean | 'mixed'
  children?: RadialMenuItem[]
}

export function buildRadialMenuModel(context: RadialMenuContext): RadialMenuItem[] {
  const noSelection = context.selectedCount === 0
  const writesDisabled = noSelection || context.readOnly || context.busy
  const compareDisabled =
    context.busy ||
    !context.compareContextAvailable ||
    context.selectedCount < 2 ||
    context.selectedCount > 4 ||
    context.selectedImageCount !== context.selectedCount
  const writeReason = (action: '标记' | '整理' | '删除') =>
    noSelection
      ? '未选择文件'
      : context.readOnly
        ? `只读项目不可${action}`
        : context.busy
          ? '请等待当前文件操作完成'
          : undefined

  const markChildren: RadialMenuItem[] = [
    markerItem('mark.keep', '保留', '✓', context.commonReview === 'keep', writesDisabled),
    markerItem('mark.pending', '待定', '•', context.commonReview === 'pending', writesDisabled),
    markerItem('mark.reject', '淘汰', '×', context.commonReview === 'reject', writesDisabled),
    markerItem('mark.clear', '清除', '○', context.commonReview === null, writesDisabled),
    {
      id: 'mark.favorite',
      label: context.commonFavorite === true ? '取消收藏' : '收藏',
      symbol: '★',
      disabled: writesDisabled,
      checked: context.commonFavorite,
    },
  ]

  const organizeChildren: RadialMenuItem[] = [
    {
      id: 'organize.rename',
      label: context.selectedCount > 1 ? '批量重命名' : '重命名',
      symbol: '✎',
      disabled: writesDisabled,
    },
    { id: 'organize.copy', label: '复制到', symbol: '⧉', disabled: writesDisabled },
    { id: 'organize.move', label: '移动到', symbol: '→', disabled: writesDisabled },
  ]

  return [
    {
      id: 'preview',
      label: '预览',
      symbol: '◉',
      disabled: context.selectedCount !== 1,
      disabledReason:
        context.selectedCount !== 1 ? '预览仅适用于单个文件' : undefined,
    },
    {
      id: 'mark',
      label: '标记',
      symbol: '★',
      disabled: writesDisabled,
      disabledReason: writeReason('标记'),
      children: markChildren,
    },
    {
      id: 'organize',
      label: '整理',
      symbol: '⇄',
      disabled: writesDisabled,
      disabledReason: writeReason('整理'),
      children: organizeChildren,
    },
    {
      id: 'trash',
      label: '移到废纸篓',
      symbol: '⌫',
      disabled: writesDisabled,
      disabledReason: writeReason('删除'),
      tone: 'destructive',
    },
    {
      id: 'compare',
      label: '并排对比',
      symbol: '▣',
      disabled: compareDisabled,
      disabledReason: context.busy
        ? '请等待当前文件操作完成'
        : !context.compareContextAvailable
          ? '请先返回文件夹内容，再选择图片进行对比'
          : '请选择 2–4 张图片',
    },
    {
      id: 'info',
      label: '信息',
      symbol: 'ⓘ',
      disabled: noSelection,
      disabledReason: noSelection ? '未选择文件' : undefined,
    },
  ]
}

function markerItem(
  id: RadialLeafAction,
  label: string,
  symbol: string,
  checked: boolean,
  disabled: boolean,
): RadialMenuItem {
  return { id, label, symbol, checked, disabled }
}
