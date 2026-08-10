import assert from 'node:assert/strict'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const verifier = fileURLToPath(new URL('./verify-runtime.sh', import.meta.url))
const linkageValidator = fileURLToPath(new URL('./validate-linkage.mjs', import.meta.url))
const rpathValidator = fileURLToPath(new URL('./validate-rpaths.mjs', import.meta.url))
const buildScript = fileURLToPath(new URL('./build-macos-runtime.sh', import.meta.url))

test('runtime verifier rejects an empty stage before native inspection', async () => {
  const stage = await mkdtemp(join(tmpdir(), 'viewer-video-stage-'))
  const result = spawnSync('/bin/bash', [verifier], {
    encoding: 'utf8',
    env: { ...process.env, VIEWER_VIDEO_STAGE_DIR: stage },
  })

  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /missing libmpv\.2\.dylib/)
})

test('linkage policy permits the approved macOS iconv library', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-linkage-'))
  const linkage = join(directory, 'linkage.txt')
  await writeFile(
    linkage,
    [
      'ViewerVideoRuntime/lib/libmpv.2.dylib:',
      '\t@rpath/libmpv.2.dylib (compatibility version 2.0.0, current version 2.3.0)',
      '\t/usr/lib/libiconv.2.dylib (compatibility version 7.0.0, current version 7.0.0)',
      '\t/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation (compatibility version 150.0.0, current version 3500.0.0)',
    ].join('\n'),
  )

  const result = spawnSync(process.execPath, [linkageValidator, linkage], { encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
})

test('linkage policy rejects non-bundled third-party libraries', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-linkage-'))
  const linkage = join(directory, 'linkage.txt')
  await writeFile(
    linkage,
    [
      'ViewerVideoRuntime/lib/libmpv.2.dylib:',
      '\t/opt/homebrew/opt/libiconv/lib/libiconv.2.dylib (compatibility version 7.0.0, current version 7.0.0)',
    ].join('\n'),
  )

  const result = spawnSync(process.execPath, [linkageValidator, linkage], { encoding: 'utf8' })
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /prohibited dynamic dependency/)
})

test('macOS runtime build explicitly links the approved system iconv', async () => {
  const source = await readFile(buildScript, 'utf8')
  assert.match(source, /"-Dc_link_args=-lc\+\+ -liconv"/)
})

test('macOS runtime build enables the backend required for VideoToolbox OpenGL interop', async () => {
  const source = await readFile(buildScript, 'utf8')
  assert.match(source, /-Dcocoa=enabled -Dgl-cocoa=enabled/)
  assert.doesNotMatch(source, /-Dcocoa=disabled|-Dgl-cocoa=disabled/)
  assert.match(source, /-Dmacos-cocoa-cb=disabled/)
})

test('bundled pkgconf does not classify the Viewer dependency prefix as a system path', async () => {
  const source = await readFile(buildScript, 'utf8')
  assert.match(
    source,
    /meson_static_install pkgconf -Dtests=disabled \\\n  -Dwith-system-includedir=\/usr\/include -Dwith-system-libdir=\/usr\/lib/,
  )
})

test('rpath policy permits only the system Swift runtime', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-rpath-'))
  const rpaths = join(directory, 'rpaths.txt')
  await writeFile(
    rpaths,
    [
      'Load command 10',
      '          cmd LC_RPATH',
      '      cmdsize 32',
      '         path /usr/lib/swift (offset 12)',
    ].join('\n'),
  )

  const result = spawnSync(process.execPath, [rpathValidator, rpaths], { encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
})

test('rpath policy rejects an Xcode or command-line-tools runtime path', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-rpath-'))
  const rpaths = join(directory, 'rpaths.txt')
  await writeFile(
    rpaths,
    [
      'Load command 10',
      '          cmd LC_RPATH',
      '      cmdsize 96',
      '         path /Library/Developer/CommandLineTools/usr/lib/swift/macosx (offset 12)',
    ].join('\n'),
  )

  const result = spawnSync(process.execPath, [rpathValidator, rpaths], { encoding: 'utf8' })
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /prohibited runtime search path/)
})

test('macOS runtime build enables Swift only for the supported mpv bridge', async () => {
  const source = await readFile(buildScript, 'utf8')
  assert.match(source, /-Dswift-build=enabled/)
  assert.match(source, /-Dcplayer=false/)
  assert.match(source, /-Dmacos-cocoa-cb=disabled/)
  assert.match(source, /install_name_tool -delete_rpath/)
})

test('macOS runtime build compiles Swift for the reviewed deployment target', async () => {
  const source = await readFile(buildScript, 'utf8')
  assert.match(source, /swift_target=arm64-apple-macos13\.0/)
  assert.match(source, /swift_target=x86_64-apple-macos13\.0/)
  assert.match(source, /"-Dswift-flags=-target \$swift_target"/)
  assert.match(source, /export OBJCFLAGS="\$CFLAGS"/)
})
