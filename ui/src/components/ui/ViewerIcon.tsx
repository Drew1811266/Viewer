import type { ImgHTMLAttributes } from 'react'

export const VIEWER_ICON_NAMES = [
  'alert-triangle',
  'arrow-down',
  'arrow-up',
  'arrow-up-right',
  'check',
  'chevron-down',
  'chevron-left',
  'chevron-right',
  'chevron-up',
  'circle',
  'circle-dot',
  'columns-2',
  'copy',
  'ellipsis',
  'eye',
  'folder-input',
  'folder-output',
  'grip-vertical',
  'info',
  'layout-grid',
  'lock',
  'maximize',
  'minus',
  'minimize',
  'move',
  'panel-right',
  'pause',
  'pencil',
  'play',
  'plus',
  'refresh-cw',
  'rotate-cw',
  'search',
  'settings',
  'sliders-horizontal',
  'skip-back',
  'skip-forward',
  'star',
  'trash-2',
  'volume-2',
  'volume-x',
  'x',
  'zoom-in',
] as const

export type ViewerIconName = (typeof VIEWER_ICON_NAMES)[number]

export interface ViewerIconProps
  extends Omit<
    ImgHTMLAttributes<HTMLImageElement>,
    'src' | 'alt' | 'width' | 'height' | 'aria-hidden'
  > {
  name: ViewerIconName
  size?: number
}

const iconUrls = import.meta.glob<string>('../../assets/icons/lucide/*.svg', {
  eager: true,
  query: '?url&no-inline',
  import: 'default',
})

export default function ViewerIcon({ name, size = 16, className, ...imageProps }: ViewerIconProps) {
  const source = iconUrls[`../../assets/icons/lucide/${name}.svg`]
  if (source === undefined) throw new Error(`Missing Viewer icon asset: ${name}`)

  return (
    <img
      {...imageProps}
      className={className === undefined ? 'viewer-icon' : `viewer-icon ${className}`}
      src={source}
      alt=""
      aria-hidden="true"
      width={size}
      height={size}
      draggable={false}
    />
  )
}
