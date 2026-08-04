export default function WorkspaceLoadingState() {
  const cards = [
    'loading-card-1',
    'loading-card-2',
    'loading-card-3',
    'loading-card-4',
    'loading-card-5',
    'loading-card-6',
    'loading-card-7',
    'loading-card-8',
  ]

  return (
    <section
      className="workspace-loading"
      role="status"
      aria-label="项目内容加载中"
      aria-busy="true"
    >
      <span className="visually-hidden">正在读取项目…</span>
      {cards.map((cardId) => (
        <div className="workspace-loading-card" aria-hidden="true" key={cardId} />
      ))}
    </section>
  )
}
