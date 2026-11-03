export const STATUS_LABELS: Record<string, string> = {
  pending: '待处理',
  changed: '已改',
  replaced: '已换',
  orphaned: '已失效',
}

export const STATE_LABELS: Record<string, string> = {
  current: '已生成 · 一致',
  stale: '计划已变',
  ungenerated: '未生成',
}

export function statusLabel(status: string): string {
  return STATUS_LABELS[status] ?? status
}
