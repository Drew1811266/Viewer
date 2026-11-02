const OPERATION_RESULT_MESSAGES: Record<string, string> = {
  conflict_skipped: '同名目标已存在',
  cancelled: '操作已取消',
  session_stale: '项目状态已变化',
  source_missing: '源文件不存在',
  destination_occupied: '目标位置已被占用',
  permission_denied: '无访问权限',
  verification_failed: '结果校验失败',
  projection_stale: '项目索引待刷新',
  backend_unavailable: '本地服务暂不可用',
  invalid_target: '目标位置无效',
}

export function operationResultMessage(code: string): string | null {
  if (code === 'renamed' || code === 'copied' || code === 'moved' || code === 'moved_to_trash') {
    return null
  }
  return OPERATION_RESULT_MESSAGES[code] ?? null
}
