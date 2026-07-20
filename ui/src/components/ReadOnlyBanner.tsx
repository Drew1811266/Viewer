interface ReadOnlyBannerProps {
  busy: boolean
  onOpenSettings: () => void
  onReselect: () => void
}

export default function ReadOnlyBanner({
  busy,
  onOpenSettings,
  onReselect,
}: ReadOnlyBannerProps) {
  return (
    <section className="read-only-banner" role="status" aria-label="只读模式">
      <div>
        <strong>只读项目</strong>
        <span>可以浏览、搜索和预览，但不能修改文件或标记。</span>
      </div>
      <div className="read-only-actions">
        <button type="button" disabled={busy} onClick={onOpenSettings}>
          打开权限设置
        </button>
        <button type="button" disabled={busy} onClick={onReselect}>
          重新选择目录
        </button>
      </div>
    </section>
  )
}
