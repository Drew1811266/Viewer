export type MarkupTool = 'point' | 'arrow' | 'brush' | 'rectangle' | 'ellipse'
export type AnnotationTool = 'browse' | MarkupTool
export type AnnotationGesture = 'click' | 'drag' | 'stroke'

export interface AnnotationToolDescriptor {
  id: MarkupTool
  label: string
  shortcut: 'P' | 'A' | 'B' | 'R' | 'O'
  icon: 'circle-dot' | 'arrow-up-right' | 'pencil' | 'maximize' | 'circle'
  gesture: AnnotationGesture
  position: number
}

export const ANNOTATION_TOOLS = [
  { id: 'point', label: '点', shortcut: 'P', icon: 'circle-dot', gesture: 'click', position: 0 },
  {
    id: 'arrow',
    label: '箭头',
    shortcut: 'A',
    icon: 'arrow-up-right',
    gesture: 'drag',
    position: 1,
  },
  { id: 'brush', label: '画笔', shortcut: 'B', icon: 'pencil', gesture: 'stroke', position: 2 },
  {
    id: 'rectangle',
    label: '矩形',
    shortcut: 'R',
    icon: 'maximize',
    gesture: 'drag',
    position: 3,
  },
  {
    id: 'ellipse',
    label: '椭圆',
    shortcut: 'O',
    icon: 'circle',
    gesture: 'drag',
    position: 4,
  },
] as const satisfies ReadonlyArray<AnnotationToolDescriptor>

export function toolForShortcut(shortcut: string): (typeof ANNOTATION_TOOLS)[number] | undefined {
  const normalized = shortcut.length === 1 ? shortcut.toUpperCase() : shortcut
  return ANNOTATION_TOOLS.find((tool) => tool.shortcut === normalized)
}
