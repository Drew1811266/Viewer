import ViewerButton from './ui/ViewerButton'
import ViewerIcon from './ui/ViewerIcon'

interface ReadOnlyBannerProps {
  busy: boolean
  onOpenSettings: () => void
  onReselect: () => void
}

export default function ReadOnlyBanner({ busy, onOpenSettings, onReselect }: ReadOnlyBannerProps) {
  return (
    <section className="read-only-banner" role="status" aria-label="只读模式">
      <div>
        <ViewerIcon name="lock" size={16} />
        <strong>只读模式</strong>
        <span>可以浏览、搜索和预览，但不能修改文件或标记。</span>
      </div>
      <div className="read-only-actions">
        <ViewerButton tone="quiet" disabled={busy} onClick={onOpenSettings}>
          权限设置
        </ViewerButton>
        <ViewerButton tone="secondary" disabled={busy} onClick={onReselect}>
          重新选择目录
        </ViewerButton>
      </div>
    </section>
  )
}
