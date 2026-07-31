export interface WorkspaceLoadingStateProps {
  bandCount?: number
}

export default function WorkspaceLoadingState({ bandCount = 3 }: WorkspaceLoadingStateProps) {
  const bands = ['loading-band-1', 'loading-band-2', 'loading-band-3'].slice(0, bandCount)
  const thumbnails = ['loading-image-1', 'loading-image-2', 'loading-image-3', 'loading-image-4']

  return (
    <section
      className="workspace-loading"
      role="status"
      aria-label="项目内容加载中"
      aria-busy="true"
    >
      <span className="visually-hidden">正在读取项目…</span>
      {bands.map((bandId) => (
        <div className="workspace-loading-band" aria-hidden="true" key={bandId}>
          <div className="workspace-loading-identity" />
          <div className="workspace-loading-track">
            {thumbnails.map((thumbnailId) => (
              <div className="workspace-loading-thumbnail" key={`${bandId}:${thumbnailId}`} />
            ))}
          </div>
        </div>
      ))}
    </section>
  )
}
