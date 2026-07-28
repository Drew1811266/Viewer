export const MIN_COMPARE_IMAGES = 2
export const MAX_COMPARE_IMAGES = 20

export interface CompareCandidateLike {
  entityId: string
  kind: string
}

export type CompareValidationReason =
  | 'invalid_cardinality'
  | 'duplicate_entity'
  | 'unsupported_type'

export type CompareValidationResult = { ok: true } | { ok: false; reason: CompareValidationReason }

export function isSupportedCompareKind(kind: string): boolean {
  return kind === 'jpeg' || kind === 'png'
}

export function validateCompareCandidates(
  candidates: readonly CompareCandidateLike[],
): CompareValidationResult {
  if (candidates.length < MIN_COMPARE_IMAGES || candidates.length > MAX_COMPARE_IMAGES) {
    return { ok: false, reason: 'invalid_cardinality' }
  }
  if (new Set(candidates.map((candidate) => candidate.entityId)).size !== candidates.length) {
    return { ok: false, reason: 'duplicate_entity' }
  }
  if (candidates.some((candidate) => !isSupportedCompareKind(candidate.kind))) {
    return { ok: false, reason: 'unsupported_type' }
  }
  return { ok: true }
}

export function compareValidationMessage(_reason: CompareValidationReason): string {
  return '请选择 2–20 张 JPG 或 PNG 图片进行对比。'
}
