import ImagePreview from '../../components/ImagePreview'
import { defined } from '../../defined'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import { ACCEPTANCE_FILES, imageRepresentation } from '../acceptanceFixtures'

const PREVIEW_FILE = defined(
  ACCEPTANCE_FILES.find(({ name }) => name === '商品-02.jpg'),
  'Missing PRE-01 image fixture',
)

export const VIEWING_SCENES: AcceptanceSceneRegistry = {
  'PRE-01': function PreviewFitScene() {
    return (
      <ImagePreview
        file={PREVIEW_FILE}
        files={ACCEPTANCE_FILES}
        requestImage={async (file, representation) =>
          imageRepresentation(file, representation.kind)
        }
        onNavigate={noOp}
        onClose={noOp}
      />
    )
  },
}

function noOp() {}
