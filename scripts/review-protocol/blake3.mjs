// Unkeyed, 32-byte BLAKE3 for the bounded protocol files. The reference algorithm
// and test vectors are published at https://github.com/BLAKE3-team/BLAKE3.
const blake3Iv = Uint32Array.from([
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
])
const blake3Permutation = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
const blake3Schedules = [Array.from({ length: 16 }, (_, index) => index)]
for (let round = 1; round < 7; round += 1) {
  const previous = blake3Schedules[round - 1]
  blake3Schedules.push(blake3Permutation.map((index) => previous[index]))
}

export function blake3Hex(bytes) {
  const hasher = createBlake3Hasher()
  hasher.update(bytes)
  return hasher.digestHex()
}

// Keep the final (possibly full) chunk until another byte arrives. Its Output,
// not just its chaining value, is required for ROOT finalization. Memory is one
// 1024-byte chunk plus one CV per level of the binary tree.
export function createBlake3Hasher() {
  const chunk = new Uint8Array(1024)
  const stack = []
  let used = 0
  let completedChunks = 0
  let digest
  return {
    update(bytes) {
      if (digest !== undefined) throw new Error('BLAKE3 hasher is finalized')
      if (!(bytes instanceof Uint8Array)) throw new TypeError('BLAKE3 input must be bytes')
      for (let offset = 0; offset < bytes.length;) {
        if (used === chunk.length) {
          let value = blake3ChainingValue(blake3ChunkOutput(chunk, completedChunks))
          completedChunks += 1
          if (!Number.isSafeInteger(completedChunks)) throw new RangeError('BLAKE3 input exceeds counter limit')
          for (let count = completedChunks; count % 2 === 0; count /= 2) {
            value = blake3ChainingValue(blake3ParentOutput(stack.pop(), value))
          }
          stack.push(value)
          used = 0
        }
        const take = Math.min(chunk.length - used, bytes.length - offset)
        chunk.set(bytes.subarray(offset, offset + take), used)
        used += take
        offset += take
      }
    },
    digestHex() {
      if (digest !== undefined) return digest
      let output = blake3ChunkOutput(chunk.subarray(0, used), completedChunks)
      while (stack.length > 0) output = blake3ParentOutput(stack.pop(), blake3ChainingValue(output))
      const words = blake3Compress({ ...output, counter: 0, flags: output.flags | 8 })
      const bytes = Buffer.alloc(32)
      for (let index = 0; index < 8; index += 1) bytes.writeUInt32LE(words[index], index * 4)
      digest = bytes.toString('hex')
      return digest
    },
  }
}

function blake3ChunkOutput(bytes, counter) {
  const blockCount = Math.max(1, Math.ceil(bytes.length / 64))
  let inputCv = blake3Iv
  let output
  for (let index = 0; index < blockCount; index += 1) {
    const block = bytes.subarray(index * 64, (index + 1) * 64)
    const blockWords = new Uint32Array(16)
    for (let offset = 0; offset < block.length; offset += 1) {
      blockWords[offset >>> 2] |= block[offset] << ((offset & 3) * 8)
    }
    output = {
      inputCv,
      blockWords,
      counter,
      blockLength: block.length,
      flags: (index === 0 ? 1 : 0) | (index === blockCount - 1 ? 2 : 0),
    }
    if (index < blockCount - 1) inputCv = blake3ChainingValue(output)
  }
  return output
}

function blake3ParentOutput(left, right) {
  const blockWords = new Uint32Array(16)
  blockWords.set(left)
  blockWords.set(right, 8)
  return { inputCv: blake3Iv, blockWords, counter: 0, blockLength: 64, flags: 4 }
}

function blake3ChainingValue(output) {
  return blake3Compress(output).slice(0, 8)
}

function blake3Compress({ inputCv, blockWords, counter, blockLength, flags }) {
  const state = new Uint32Array(16)
  state.set(inputCv)
  state.set(blake3Iv.subarray(0, 4), 8)
  state[12] = counter >>> 0
  state[13] = Math.floor(counter / 0x1_0000_0000)
  state[14] = blockLength
  state[15] = flags
  for (const schedule of blake3Schedules) {
    const word = (index) => blockWords[schedule[index]]
    blake3Mix(state, 0, 4, 8, 12, word(0), word(1))
    blake3Mix(state, 1, 5, 9, 13, word(2), word(3))
    blake3Mix(state, 2, 6, 10, 14, word(4), word(5))
    blake3Mix(state, 3, 7, 11, 15, word(6), word(7))
    blake3Mix(state, 0, 5, 10, 15, word(8), word(9))
    blake3Mix(state, 1, 6, 11, 12, word(10), word(11))
    blake3Mix(state, 2, 7, 8, 13, word(12), word(13))
    blake3Mix(state, 3, 4, 9, 14, word(14), word(15))
  }
  for (let index = 0; index < 8; index += 1) {
    state[index] ^= state[index + 8]
    state[index + 8] ^= inputCv[index]
  }
  return state
}

function blake3Mix(state, a, b, c, d, x, y) {
  state[a] = state[a] + state[b] + x
  state[d] = rotateRight(state[d] ^ state[a], 16)
  state[c] = state[c] + state[d]
  state[b] = rotateRight(state[b] ^ state[c], 12)
  state[a] = state[a] + state[b] + y
  state[d] = rotateRight(state[d] ^ state[a], 8)
  state[c] = state[c] + state[d]
  state[b] = rotateRight(state[b] ^ state[c], 7)
}

function rotateRight(value, amount) {
  return (value >>> amount) | (value << (32 - amount))
}
