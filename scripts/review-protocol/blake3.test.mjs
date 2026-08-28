import assert from 'node:assert/strict'
import test from 'node:test'
import { blake3Hex, createBlake3Hasher } from './blake3.mjs'

// Literal 32-byte prefixes from BLAKE3's official i % 251 vectors, not a second
// invocation of the implementation under test.
const vectors = [
  [0, 'af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262'],
  [64, '4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98'],
  [65, 'de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee'],
  [1024, '42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7'],
  [1025, 'd00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444'],
  [2048, 'e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a'],
  [3072, 'b98cb0ff3623be03326b373de6b9095218513e64f1ee2edd2525c7ad1e5cffd2'],
  [5121, '628bd2cb2004694adaab7bbd778a25df25c47b9d4155a55f8fbd79f2fe154cff'],
]

test('incremental BLAKE3 preserves block, chunk, tree and root semantics', () => {
  for (const [length, expected] of vectors) {
    const input = Uint8Array.from({ length }, (_, i) => i % 251)
    assert.equal(blake3Hex(input), expected)
    for (const step of [1, 63, 64, 65, 511, 1024, 1025, 2048, 8192]) {
      const hasher = createBlake3Hasher()
      for (let offset = 0; offset < input.length; offset += step) {
        hasher.update(input.subarray(offset, offset + step))
        hasher.update(new Uint8Array())
      }
      assert.equal(hasher.digestHex(), expected, `${length} bytes, ${step} chunks`)
      assert.equal(hasher.digestHex(), expected, 'digest is repeatable')
      assert.throws(() => hasher.update(new Uint8Array()), /finalized/)
    }
  }
})

test('incremental hashing owns the partial input and rejects non-byte values', () => {
  const hasher = createBlake3Hasher()
  const bytes = Buffer.from('abc')
  hasher.update(bytes)
  bytes.fill(0)
  assert.equal(hasher.digestHex(), '6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85')
  assert.throws(() => blake3Hex('abc'), TypeError)
  assert.throws(() => createBlake3Hasher().update([1]), TypeError)
})
