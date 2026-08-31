import type { ProjectAccess } from '../../app/review/reviewModel'
import type { ContinuousReviewCoordinator } from '../../app/review/useContinuousReviewCoordinator'

interface ContinuousReviewRecoveryNoticeProps {
  coordinator: ContinuousReviewCoordinator
  projectAccess: ProjectAccess
}

interface NoticeModel {
  title: string
  detail: string
  role: 'alert' | 'status'
  tone?: 'danger' | 'warning'
  live?: 'polite'
}

const READ_ONLY_NOTICE: NoticeModel = {
  title: '持续评审为只读',
  detail: '当前项目不能迁移、导入或保存意见；历史和普通预览仍可查看。',
  role: 'status',
  tone: 'warning',
}

export default function ContinuousReviewRecoveryNotice({
  coordinator,
  projectAccess,
}: ContinuousReviewRecoveryNoticeProps) {
  const notice = continuousNotice(coordinator, projectAccess)
  if (notice === null) return null
  return (
    <section
      className="review-local-notice"
      data-tone={notice.tone}
      role={notice.role}
      aria-live={notice.live}
    >
      <div>
        <strong>{notice.title}</strong>
        <span>{notice.detail}</span>
      </div>
    </section>
  )
}

function continuousNotice(
  coordinator: ContinuousReviewCoordinator,
  projectAccess: ProjectAccess,
): NoticeModel | null {
  const { state, error } = coordinator
  if (state.kind === 'loading') {
    return {
      title: '正在核验评审协议',
      detail: '核验完成前不会启用旧写入或新评审操作。',
      role: 'status',
      live: 'polite',
    }
  }
  if (state.kind === 'recovery_required') {
    return {
      title: '持续评审记录需要恢复处理',
      detail: '已提交版本保持可读；Viewer 不会猜测迁移或重复写入。',
      role: 'alert',
      tone: 'danger',
    }
  }
  if (continuousReadOnly(coordinator, projectAccess)) return READ_ONLY_NOTICE
  if (state.kind === 'unavailable' || error !== null) {
    const unavailable = state.kind === 'unavailable' ? state.error : error
    return {
      title: unavailableTitle(unavailable?.code),
      detail: unavailable?.message ?? '普通浏览和预览仍可继续。',
      role: 'alert',
      tone: 'warning',
    }
  }
  if (state.kind === 'migration_required' || coordinator.view?.migration) return null
  if (!coordinator.view?.capabilities.continuousEditing) return READ_ONLY_NOTICE
  return null
}

function continuousReadOnly(
  coordinator: ContinuousReviewCoordinator,
  projectAccess: ProjectAccess,
): boolean {
  const { state, error } = coordinator
  return (
    projectAccess === 'read_only' ||
    (state.kind === 'unavailable' && state.error.code === 'read_only') ||
    error?.code === 'read_only'
  )
}

function unavailableTitle(code: string | undefined): string {
  return code === 'unsupported_protocol' ? '评审协议版本暂不支持' : '持续评审暂不可用'
}
