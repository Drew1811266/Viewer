interface ImagePreviewLoadingProps {
  visible: boolean
}

export default function ImagePreviewLoading({ visible }: ImagePreviewLoadingProps) {
  return (
    <section
      className="image-preview-loading"
      data-testid="image-preview-loading"
      data-visible={visible}
      aria-hidden={visible ? undefined : true}
      role="status"
    >
      <strong>正在载入图片</strong>
      <p>正在准备高清预览…</p>
      <div
        className="image-preview-loading__progress"
        role="progressbar"
        aria-label="正在准备高清预览"
      >
        <span className="image-preview-loading__indicator" />
      </div>
    </section>
  )
}
