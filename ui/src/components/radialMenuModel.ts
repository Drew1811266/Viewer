import type { ReviewState } from '../api/types'
import { MAX_COMPARE_IMAGES, MIN_COMPARE_IMAGES } from '../state/comparePolicy'
import type { ViewerIconName } from './ui/ViewerIcon'

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

export type RadialPrimaryId = 'preview' | 'mark' | 'organize' | 'trash' | 'compare' | 'info'

export interface RadialMenuContext {
  selectedCount: number
  selectedImageCount: number
  previewEnabled: boolean
  previewDisabledReason?: string
  readOnly: boolean
  busy: boolean
  compareContextAvailable: boolean
  commonReview: ReviewState | null | 'mixed'
  commonFavorite: boolean | 'mixed'
}

export interface RadialMenuItem {
  id: RadialPrimaryId | RadialLeafAction
  label: string
  icon: ViewerIconName
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
    context.selectedCount < MIN_COMPARE_IMAGES ||
    context.selectedCount > MAX_COMPARE_IMAGES ||
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
    markerItem('mark.keep', '保留', 'check', context.commonReview === 'keep', writesDisabled),
    markerItem(
      'mark.pending',
      '待定',
      'circle-dot',
      context.commonReview === 'pending',
      writesDisabled,
    ),
    markerItem('mark.reject', '淘汰', 'x', context.commonReview === 'reject', writesDisabled),
    markerItem('mark.clear', '清除', 'circle', context.commonReview === null, writesDisabled),
    {
      id: 'mark.favorite',
      label:
        context.commonFavorite === true
          ? '取消收藏'
          : context.commonFavorite === 'mixed'
            ? '切换收藏'
            : '收藏',
      icon: 'star',
      disabled: writesDisabled,
      checked: context.commonFavorite,
    },
  ]

  const organizeChildren: RadialMenuItem[] = [
    {
      id: 'organize.rename',
      label: context.selectedCount > 1 ? '批量重命名' : '重命名',
      icon: 'pencil',
      disabled: writesDisabled,
    },
    { id: 'organize.copy', label: '复制到', icon: 'copy', disabled: writesDisabled },
    { id: 'organize.move', label: '移动到', icon: 'folder-output', disabled: writesDisabled },
  ]

  return [
    {
      id: 'preview',
      label: '预览',
      icon: 'eye',
      disabled: !context.previewEnabled,
      disabledReason: context.previewDisabledReason,
    },
    {
      id: 'mark',
      label: '标记',
      icon: 'star',
      disabled: writesDisabled,
      disabledReason: writeReason('标记'),
      children: markChildren,
    },
    {
      id: 'organize',
      label: '整理',
      icon: 'folder-input',
      disabled: writesDisabled,
      disabledReason: writeReason('整理'),
      children: organizeChildren,
    },
    {
      id: 'trash',
      label: '移到废纸篓',
      icon: 'trash-2',
      disabled: writesDisabled,
      disabledReason: writeReason('删除'),
      tone: 'destructive',
    },
    {
      id: 'compare',
      label: '并排对比',
      icon: 'columns-2',
      disabled: compareDisabled,
      disabledReason: context.busy
        ? '请等待当前文件操作完成'
        : !context.compareContextAvailable
          ? '请先返回文件夹内容，再选择图片进行对比'
          : context.selectedCount > MAX_COMPARE_IMAGES
            ? `最多同时对比 ${MAX_COMPARE_IMAGES} 张图片`
            : context.selectedCount < MIN_COMPARE_IMAGES
              ? `请选择 ${MIN_COMPARE_IMAGES}–${MAX_COMPARE_IMAGES} 张图片`
              : context.selectedImageCount !== context.selectedCount
                ? '仅支持图片'
                : undefined,
    },
    {
      id: 'info',
      label: '信息',
      icon: 'info',
      disabled: noSelection,
      disabledReason: noSelection ? '未选择文件' : undefined,
    },
  ]
}

function markerItem(
  id: RadialLeafAction,
  label: string,
  icon: ViewerIconName,
  checked: boolean,
  disabled: boolean,
): RadialMenuItem {
  return { id, label, icon, checked, disabled }
}
