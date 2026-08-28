import { constants } from 'node:fs'
import { lstat, open, opendir, realpath } from 'node:fs/promises'
import path from 'node:path'
import { blake3Hex, createBlake3Hasher } from './blake3.mjs'

export const MAX_JSON_BYTES = 64 * 1024 * 1024
export const MAX_PNG_BYTES = 64 * 1024 * 1024
export const MAX_PNG_PIXELS = 16_777_216
const BUFFER_BYTES = 64 * 1024
const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])

export class SafeReadError extends Error {
  constructor(code, message, fsCode) {
    super(message)
    this.code = code
    this.fsCode = fsCode
  }
}
const fail = (code, message) => { throw new SafeReadError(code, message) }
const sameIdentity = (a, b) => a.dev === b.dev && a.ino === b.ino
const sameFile = (a, b) => sameIdentity(a, b) && a.size === b.size
  && a.mtimeNs === b.mtimeNs && a.ctimeNs === b.ctimeNs && a.nlink === b.nlink

function portablePath(value) {
  return typeof value === 'string' && value.length > 0 && Buffer.byteLength(value) <= 4096
    && !value.startsWith('/') && !value.includes('\\') && !value.includes('\0')
    && !/^[A-Za-z]:/.test(value)
    && value.split('/').every(part => part !== '' && part !== '.' && part !== '..')
}
export function isSafeSourcePath(value) {
  return portablePath(value) && value.split('/').every(part => part.toLowerCase() !== '.viewer')
}
export function isSafeRepositoryLocation(value) {
  // Repository locations are relative to the private reviews root, never to the
  // project root. They cannot switch to source-file authority (or another .viewer).
  return portablePath(value) && value.split('/').every(part => part.toLowerCase() !== '.viewer')
}

async function metadata(file, label, directory) {
  const value = await lstat(file, { bigint: true })
  if (value.isSymbolicLink()) fail('unsafe_path', `${label} cannot be a symbolic link`)
  if (directory ? !value.isDirectory() : !value.isFile()) {
    fail('integrity', `${label} must be a ${directory ? 'directory' : 'regular file'}`)
  }
  return value
}
function ioError(error, label) {
  if (error instanceof SafeReadError) return error
  return new SafeReadError(error?.code === 'ELOOP' ? 'unsafe_path' : 'io', `${label} is unavailable`, error?.code)
}

export async function openSafeProject(projectRoot) {
  if (typeof projectRoot !== 'string' || !projectRoot) fail('unsafe_path', 'project root is required')
  // No fallback to following a final symlink or blocking on a substituted FIFO.
  // Node fs flags are platform-dependent: fail closed if the safe flags do not exist.
  if (constants.O_NOFOLLOW === undefined || constants.O_NONBLOCK === undefined) {
    fail('io', 'safe file opening is unavailable on this platform')
  }
  let root
  let identity
  const ancestors = []
  const requestedRoot = path.resolve(projectRoot)
  try {
    const before = await metadata(requestedRoot, 'project root', true)
    root = await realpath(requestedRoot)
    identity = await metadata(root, 'project root', true)
    if (!sameIdentity(before, identity)) fail('integrity', 'project root identity changed')
    for (let directory = path.dirname(root); ; directory = path.dirname(directory)) {
      ancestors.unshift([directory, await metadata(directory, 'project ancestor', true)])
      if (directory === path.dirname(directory)) break
    }
  } catch (error) { throw ioError(error, 'project root') }

  async function checkRoot() {
    const current = await metadata(root, 'project root', true)
    if (!sameIdentity(identity, current)
        || !sameIdentity(identity, await metadata(requestedRoot, 'project root', true))
        || await realpath(requestedRoot) !== root || await realpath(root) !== root) {
      fail('integrity', 'project root identity changed')
    }
    return current
  }
  async function locate(relative, label, directory = false) {
    const directories = []
    for (const [ancestor, pinned] of ancestors) {
      const before = await metadata(ancestor, 'project ancestor', true)
      if (!sameIdentity(before, pinned)) fail('integrity', 'project ancestor identity changed')
      directories.push([ancestor, before])
    }
    const rootBefore = await checkRoot()
    let current = root
    directories.push([root, rootBefore])
    const parts = relative.split('/')
    for (const [i, part] of parts.entries()) {
      current = path.join(current, part)
      const isDirectory = i < parts.length - 1 || directory
      const stat = await metadata(current, label, isDirectory)
      if (isDirectory) directories.push([current, stat])
      else return { file: current, stat, directories }
    }
    return { file: current, directories }
  }
  async function revalidate(located, label) {
    await checkRoot()
    for (const [index, [directory, before]] of located.directories.entries()) {
      const after = await metadata(directory, label, true)
      if (!sameIdentity(before, after)) {
        fail('integrity', `${label} directory identity changed during read`)
      }
      // Node has no portable openat. Parent mutation timestamps also detect a
      // directory swapped out and back between individual lstat/open calls.
      // The final containing directory may gain files during normal publication;
      // its own replacement changes the timestamp of its checked parent.
      if (index < located.directories.length - 1
          && (before.ctimeNs !== after.ctimeNs || before.mtimeNs !== after.mtimeNs)) {
        const error = new SafeReadError('integrity', `${label} parent directory changed during read`)
        error.retryableDirectoryChange = true
        throw error
      }
    }
  }
  async function read(relative, maximumBytes, label, sourceHash = false, mutableIndex = false) {
    let handle
    try {
      const located = await locate(relative, label)
      if (!sourceHash && located.stat.nlink !== 1n) fail('integrity', `${label} cannot have multiple hard links`)
      if (located.stat.size > BigInt(maximumBytes)) fail('limit_exceeded', `${label} exceeds its size limit`)
      handle = await open(located.file, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK)
      const before = await handle.stat({ bigint: true })
      if (!before.isFile()) fail('integrity', `${label} must be a regular file`)
      if (!sameFile(located.stat, before)) fail('integrity', `${label} identity changed while opening`)
      await revalidate(located, label)
      // Source reads never retain chunks. JSON/PNG have a separate explicit cap.
      const bytes = sourceHash ? null : Buffer.alloc(Number(before.size))
      const buffer = Buffer.allocUnsafe(BUFFER_BYTES)
      const hash = sourceHash ? createBlake3Hasher() : null
      let total = 0
      for (;;) {
        const { bytesRead } = await handle.read(buffer, 0, buffer.length, null)
        if (!bytesRead) break
        total += bytesRead
        if (total > maximumBytes || BigInt(total) > before.size) fail('integrity', `${label} size changed during read`)
        if (hash) hash.update(buffer.subarray(0, bytesRead))
        else buffer.copy(bytes, total - bytesRead, 0, bytesRead)
      }
      const after = await handle.stat({ bigint: true })
      await revalidate(located, label)
      const atPath = await metadata(located.file, label, false)
      const publishedNewIndex = mutableIndex && after.nlink === 0n && !sameIdentity(after, atPath)
        && sameIdentity(before, after) && before.size === after.size && before.mtimeNs === after.mtimeNs
      if (publishedNewIndex) {
        // Atomic rename may unlink the fixed old index. Re-read that SAME handle
        // to reject torn bytes without switching to the new published version.
        let verified = 0
        while (verified < total) {
          const { bytesRead } = await handle.read(buffer, 0, Math.min(buffer.length, total - verified), verified)
          if (!bytesRead || !buffer.subarray(0, bytesRead).equals(bytes.subarray(verified, verified + bytesRead))) {
            fail('integrity', `${label} contents changed during read`)
          }
          verified += bytesRead
        }
        if (!sameFile(after, await handle.stat({ bigint: true }))) fail('integrity', `${label} contents changed during read`)
      }
      if (BigInt(total) !== before.size || (!publishedNewIndex && (!sameFile(before, after) || !sameFile(after, atPath)))) {
        fail('integrity', `${label} identity or contents changed during read`)
      }
      await revalidate(located, label)
      return hash ? { blake3: hash.digestHex(), sizeBytes: total } : bytes
    } catch (error) { throw ioError(error, label) }
    finally { await handle?.close() }
  }
  async function repositoryBytes(location, maximumBytes, label = 'repository object') {
    if (!isSafeRepositoryLocation(location)) fail('unsafe_path', `${label} location is invalid`)
    if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 0 || maximumBytes > MAX_JSON_BYTES) {
      fail('limit_exceeded', `${label} size limit is invalid`)
    }
    for (let attempt = 0; ; attempt += 1) {
      try { return await read(`.viewer/reviews/${location}`, maximumBytes, label, false, location === 'index.json') }
      catch (error) {
        // Retry the same exact location only; callers retain their selected
        // SnapshotRef and digest. Sustained mutations fail closed after 3 tries.
        if (!error.retryableDirectoryChange || attempt === 2) throw error
      }
    }
  }
  return Object.freeze({
    readRepositoryBytes: repositoryBytes,
    async readRepositoryJson(location, maximumBytes, label = 'repository JSON') {
      const bytes = await repositoryBytes(location, maximumBytes, label)
      let data
      try { data = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) }
      catch { fail('integrity', `${label} is not valid JSON`) }
      return { bytes, data }
    },
    async readRepositoryPng(location, expected, label = 'review artifact', maximumBytes = MAX_PNG_BYTES) {
      const bytes = await repositoryBytes(location, Math.min(MAX_PNG_BYTES, maximumBytes), label)
      if (expected.sizeBytes !== undefined && bytes.length !== expected.sizeBytes) fail('integrity', `${label} size does not match its manifest`)
      if (blake3Hex(bytes) !== expected.blake3) fail('integrity', `${label} digest does not match its manifest`)
      const dimensions = pngDimensions(bytes, label)
      if (dimensions.width !== expected.width || dimensions.height !== expected.height) fail('integrity', `${label} dimensions do not match its manifest`)
      return { bytes, ...dimensions }
    },
    async readRepositoryDirectory(location, maximumEntries, label) {
      if (location !== '' && !isSafeRepositoryLocation(location)) fail('unsafe_path', `${label} location is invalid`)
      if (!Number.isSafeInteger(maximumEntries) || maximumEntries < 0 || maximumEntries > 50_000) fail('limit_exceeded', 'directory entry limit is invalid')
      try {
        const located = await locate(`.viewer/reviews${location ? `/${location}` : ''}`, label, true)
        const entries = []
        for await (const entry of await opendir(located.file)) {
          if (entry.isSymbolicLink()) fail('unsafe_path', `${label} cannot contain a symbolic link`)
          entries.push(entry)
          if (entries.length > maximumEntries) fail('limit_exceeded', `${label} exceeds its entry limit`)
        }
        await revalidate(located, label)
        return entries
      } catch (error) { throw ioError(error, label) }
    },
    async checkSource(relativePath, expectedFingerprint) {
      let status
      try {
        if (!isSafeSourcePath(relativePath) || !/^[0-9a-f]{64}$/.test(expectedFingerprint?.blake3 ?? '')
            || !Number.isSafeInteger(expectedFingerprint?.sizeBytes) || expectedFingerprint.sizeBytes < 0) {
          status = 'unverified'
        } else {
          const observed = await read(relativePath, Number.MAX_SAFE_INTEGER, 'source', true)
          status = observed.blake3 === expectedFingerprint.blake3 && observed.sizeBytes === expectedFingerprint.sizeBytes ? 'match' : 'changed'
        }
      } catch (error) {
        status = error.fsCode === 'ENOENT' ? 'missing'
          : error.code === 'unsafe_path' || /changed/.test(error.message) ? 'unverified' : 'unreadable'
      }
      return { checkedAtMs: Date.now(), status }
    },
  })
}

export function pngDimensions(bytes, label) {
  if (bytes.length < 24 || !bytes.subarray(0, 8).equals(signature) || bytes.toString('ascii', 12, 16) !== 'IHDR') {
    fail('integrity', `${label} is not a PNG image`)
  }
  const width = bytes.readUInt32BE(16)
  const height = bytes.readUInt32BE(20)
  if (!width || !height || width * height > MAX_PNG_PIXELS) fail('integrity', `${label} dimensions are invalid`)
  return { width, height }
}
