export type MarkupTool =
  | 'point'
  | 'arrow'
  | 'brush'
  | 'rectangle'
  | 'ellipse'
  | 'text'
  | 'line'
  | 'highlighter'
  | 'polygon'
  | 'ruler'
export type AnnotationTool = 'browse' | MarkupTool
export type AnnotationGesture = 'click' | 'drag' | 'stroke' | 'polygon'

export interface AnnotationToolDescriptor {
  id: MarkupTool
  label: string
  shortcut: 'P' | 'A' | 'B' | 'R' | 'O' | 'T' | 'L' | 'H' | 'G' | 'M'
  icon:
    | 'circle-dot'
    | 'arrow-up-right'
    | 'pencil'
    | 'maximize'
    | 'circle'
    | 'type'
    | 'line'
    | 'highlighter'
    | 'pentagon'
    | 'ruler'
  gesture: AnnotationGesture
  position: number
  instruction: string
}

export const ANNOTATION_TOOLS = [
  {
    id: 'point',
    label: '点',
    shortcut: 'P',
    icon: 'circle-dot',
    gesture: 'click',
    position: 0,
    instruction: '点击画布放置点',
  },
  {
    id: 'arrow',
    label: '箭头',
    shortcut: 'A',
    icon: 'arrow-up-right',
    gesture: 'drag',
    position: 1,
    instruction: '从起点拖到终点',
  },
  {
    id: 'brush',
    label: '画笔',
    shortcut: 'B',
    icon: 'pencil',
    gesture: 'stroke',
    position: 2,
    instruction: '按住并拖动绘制',
  },
  {
    id: 'rectangle',
    label: '矩形',
    shortcut: 'R',
    icon: 'maximize',
    gesture: 'drag',
    position: 3,
    instruction: '拖出矩形区域',
  },
  {
    id: 'ellipse',
    label: '椭圆',
    shortcut: 'O',
    icon: 'circle',
    gesture: 'drag',
    position: 4,
    instruction: '拖出椭圆区域',
  },
  {
    id: 'text',
    label: '文字',
    shortcut: 'T',
    icon: 'type',
    gesture: 'click',
    position: 5,
    instruction: '点击画布输入文字',
  },
  {
    id: 'line',
    label: '直线',
    shortcut: 'L',
    icon: 'line',
    gesture: 'drag',
    position: 6,
    instruction: '从起点拖到终点',
  },
  {
    id: 'highlighter',
    label: '荧光笔',
    shortcut: 'H',
    icon: 'highlighter',
    gesture: 'stroke',
    position: 7,
    instruction: '按住并拖动高亮',
  },
  {
    id: 'polygon',
    label: '多边形',
    shortcut: 'G',
    icon: 'pentagon',
    gesture: 'polygon',
    position: 8,
    instruction: '连续点击添加顶点',
  },
  {
    id: 'ruler',
    label: '标尺',
    shortcut: 'M',
    icon: 'ruler',
    gesture: 'drag',
    position: 9,
    instruction: '从起点拖到终点测量',
  },
] as const satisfies ReadonlyArray<AnnotationToolDescriptor>

export type AnnotationToolId = (typeof ANNOTATION_TOOLS)[number]['id']

export const LEGACY_ANNOTATION_TOOLS = ANNOTATION_TOOLS.slice(
  0,
  5,
) as ReadonlyArray<AnnotationToolDescriptor>

export function toolsForProtocol(protocol: 'legacy' | 'continuous') {
  return protocol === 'continuous' ? ANNOTATION_TOOLS : LEGACY_ANNOTATION_TOOLS
}

export function toolForShortcut(
  shortcut: string,
  tools: ReadonlyArray<AnnotationToolDescriptor> = ANNOTATION_TOOLS,
): AnnotationToolDescriptor | undefined {
  const normalized = shortcut.length === 1 ? shortcut.toUpperCase() : shortcut
  return tools.find((tool) => tool.shortcut === normalized)
}

export function isMarkupTool(tool: AnnotationTool): tool is MarkupTool {
  return tool !== 'browse'
}
