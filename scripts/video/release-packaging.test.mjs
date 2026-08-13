import assert from 'node:assert/strict'
import { chmod, copyFile, mkdir, mkdtemp, readFile, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { spawnSync } from 'node:child_process'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const scripts = dirname(fileURLToPath(import.meta.url))
const repository = dirname(dirname(scripts))
const stageScript = join(scripts, 'stage-tauri-resources.sh')
const bundleVerifier = join(scripts, 'verify-app-bundle.sh')
const licenseVerifier = join(scripts, 'verify-licenses.sh')

const run = (script, args = [], env = {}) =>
  spawnSync('/bin/bash', [script, ...args], {
    cwd: repository,
    encoding: 'utf8',
    env: { ...process.env, ...env },
  })

test('stage verifies before copying and preserves executable files', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-stage-copy-'))
  const source = join(directory, 'source')
  const destination = join(directory, 'destination')
  const verifier = join(directory, 'verify.sh')
  await mkdir(join(source, 'bin'), { recursive: true })
  await mkdir(join(source, 'lib'))
  await mkdir(join(source, 'licenses'))
  await copyFile('/usr/bin/true', join(source, 'bin/ffprobe'))
  await copyFile('/usr/bin/true', join(source, 'bin/ffmpeg'))
  await copyFile('/usr/bin/true', join(source, 'lib/libmpv.2.dylib'))
  await chmod(join(source, 'bin/ffprobe'), 0o755)
  await chmod(join(source, 'bin/ffmpeg'), 0o755)
  await writeFile(join(source, 'runtime.lock.json'), '{}\n')
  await writeFile(
    join(source, 'runtime.inventory.sha256'),
    [
      `${'0'.repeat(64)}  bin/ffmpeg`,
      `${'0'.repeat(64)}  bin/ffprobe`,
      `${'0'.repeat(64)}  lib/libmpv.2.dylib`,
      '',
    ].join('\n'),
  )
  await writeFile(verifier, '#!/bin/bash\ntest -x "$VIEWER_VIDEO_STAGE_DIR/bin/ffprobe"\n')
  await chmod(verifier, 0o755)

  const result = run(stageScript, [source, destination], {
    VIEWER_VIDEO_RUNTIME_VERIFIER: verifier,
    VIEWER_VIDEO_SIGNING_IDENTITY: 'Developer ID Application: Fixture (TEAMID)',
    VIEWER_VIDEO_CODESIGN_BIN: '/usr/bin/true',
  })

  assert.equal(result.status, 0, result.stderr)
  assert.equal((await stat(join(destination, 'bin/ffprobe'))).mode & 0o111, 0o111)
  assert.match(await readFile(join(destination, 'runtime.inventory.sha256'), 'utf8'), /bin\/ffprobe/)
})

test('stage rejects an invalid source without touching the target', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-stage-invalid-'))
  const source = join(directory, 'source')
  const destination = join(directory, 'destination')
  await mkdir(source)
  await mkdir(destination)
  await mkdir(join(source, 'bin'))
  await mkdir(join(source, 'lib'))
  await mkdir(join(source, 'licenses'))
  await writeFile(join(destination, 'sentinel'), 'keep')

  const result = run(stageScript, [source, destination])

  assert.notEqual(result.status, 0)
  assert.equal(await readFile(join(destination, 'sentinel'), 'utf8'), 'keep')
})

test('stage rejects unexpected files already present in the target', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-stage-extra-'))
  const source = join(directory, 'source')
  const destination = join(directory, 'destination')
  const verifier = join(directory, 'verify.sh')
  await mkdir(source)
  await mkdir(destination)
  await writeFile(join(source, 'expected'), 'allowed')
  await writeFile(join(source, 'runtime.lock.json'), '{}\n')
  await writeFile(
    join(source, 'runtime.inventory.sha256'),
    `${'0'.repeat(64)}  expected\n`,
  )
  await writeFile(join(destination, 'unexpected'), 'reject')
  await writeFile(verifier, '#!/bin/bash\nexit 0\n')
  await chmod(verifier, 0o755)

  const result = run(stageScript, [source, destination], {
    VIEWER_VIDEO_RUNTIME_VERIFIER: verifier,
    VIEWER_VIDEO_SIGNING_IDENTITY: 'Developer ID Application: Fixture (TEAMID)',
  })

  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /unexpected target (?:file|entry)/)
})

test('stage rejects an unexpected file in an otherwise verified source', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-stage-source-extra-'))
  const source = join(directory, 'source')
  const destination = join(directory, 'destination')
  const verifier = join(directory, 'verify.sh')
  await mkdir(source)
  await writeFile(join(source, 'runtime.lock.json'), '{}\n')
  await writeFile(join(source, 'runtime.inventory.sha256'), '')
  await writeFile(join(source, 'unexpected'), 'reject')
  await writeFile(verifier, '#!/bin/bash\nexit 0\n')
  await chmod(verifier, 0o755)

  const result = run(stageScript, [source, destination], {
    VIEWER_VIDEO_RUNTIME_VERIFIER: verifier,
    VIEWER_VIDEO_SIGNING_IDENTITY: 'Developer ID Application: Fixture (TEAMID)',
  })

  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /unexpected source file/)
})

test('stage rejects a target symlink before copying', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-stage-symlink-'))
  const source = join(directory, 'source')
  const destination = join(directory, 'destination')
  const verifier = join(directory, 'verify.sh')
  await mkdir(source)
  await mkdir(destination)
  await writeFile(join(source, 'expected'), 'allowed')
  await writeFile(join(source, 'runtime.lock.json'), '{}\n')
  await writeFile(join(source, 'runtime.inventory.sha256'), `${'0'.repeat(64)}  expected\n`)
  await writeFile(join(directory, 'outside'), 'must-not-touch')
  const { symlink } = await import('node:fs/promises')
  await symlink(join(directory, 'outside'), join(destination, 'expected'))
  await writeFile(verifier, '#!/bin/bash\nexit 0\n')
  await chmod(verifier, 0o755)

  const result = run(stageScript, [source, destination], {
    VIEWER_VIDEO_RUNTIME_VERIFIER: verifier,
    VIEWER_VIDEO_SIGNING_IDENTITY: 'Developer ID Application: Fixture (TEAMID)',
  })

  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /unexpected target entry|target symlink/)
  assert.equal(await readFile(join(directory, 'outside'), 'utf8'), 'must-not-touch')
})

test('bundle verifier rejects a missing runtime before native tool inspection', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-app-'))
  const app = join(directory, 'Viewer.app')
  await mkdir(join(app, 'Contents/Resources'), { recursive: true })

  const result = run(bundleVerifier, [app])

  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /missing bundled video runtime/)
})

async function nativeBundleFixture(dependency) {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-native-bundle-'))
  const app = join(directory, 'Viewer.app')
  const runtime = join(app, 'Contents/Resources/ViewerVideoRuntime')
  const toolDirectory = join(directory, 'tools')
  await mkdir(join(app, 'Contents/MacOS'), { recursive: true })
  await mkdir(join(runtime, 'bin'), { recursive: true })
  await mkdir(join(runtime, 'lib'))
  await mkdir(join(runtime, 'licenses'))
  await mkdir(toolDirectory)
  for (const executable of [
    join(app, 'Contents/MacOS/Viewer'),
    join(runtime, 'bin/ffmpeg'),
    join(runtime, 'bin/ffprobe'),
  ]) {
    await writeFile(executable, '#!/bin/bash\nexit 0\n')
    await chmod(executable, 0o755)
  }
  await writeFile(join(runtime, 'lib/libmpv.2.dylib'), 'fixture')
  await writeFile(join(runtime, 'runtime.lock.json'), '{}\n')
  await writeFile(join(runtime, 'runtime.inventory.sha256'), '')
  const verifier = join(directory, 'verify-runtime.sh')
  await writeFile(verifier, '#!/bin/bash\nexit 0\n')
  await chmod(verifier, 0o755)

  const tools = {
    lipo: '#!/bin/bash\nprintf "arm64\\n"\n',
    otool: `#!/bin/bash
if [[ "$1" == "-L" ]]; then
  printf '%s:\\n\\t%s (compatibility version 1.0.0, current version 1.0.0)\\n' "$2" '${dependency}'
elif [[ "$1" == "-D" ]]; then
  if [[ "$2" == *.dylib ]]; then
    printf '%s:\\n@rpath/%s\\n' "$2" "$(basename "$2")"
  else
    exit 1
  fi
else
  printf 'Load command 1\\n          cmd LC_RPATH\\n      cmdsize 48\\n         path @loader_path/../lib (offset 12)\\n'
fi
`,
    codesign: `#!/bin/bash
if [[ "$1" == "-dv" ]]; then
  printf 'Authority=Developer ID Application: Fixture (TEAMID)\\nTimestamp=Aug 12, 2026 at 12:00:00\\n' >&2
fi
exit 0
`,
    spctl: '#!/bin/bash\nexit 0\n',
    'sandbox-exec': '#!/bin/bash\nshift 2\nexec "$@"\n',
  }
  for (const [name, source] of Object.entries(tools)) {
    const path = join(toolDirectory, name)
    await writeFile(path, source)
    await chmod(path, 0o755)
  }
  return { app, toolDirectory, verifier }
}

test('bundle verifier accepts only signed architecture-matched offline runtime files', async () => {
  const fixture = await nativeBundleFixture('@rpath/libmpv.2.dylib')
  const result = run(bundleVerifier, [fixture.app], {
    PATH: `${fixture.toolDirectory}:${process.env.PATH}`,
    VIEWER_VIDEO_RUNTIME_VERIFIER: fixture.verifier,
  })

  assert.equal(result.status, 0, result.stderr)
})

test('bundle verifier rejects a host-linked runtime dependency', async () => {
  const fixture = await nativeBundleFixture('/opt/homebrew/lib/libcodec.dylib')
  const result = run(bundleVerifier, [fixture.app], {
    PATH: `${fixture.toolDirectory}:${process.env.PATH}`,
    VIEWER_VIDEO_RUNTIME_VERIFIER: fixture.verifier,
  })

  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /host-path dependency/)
})

test('bundle verifier rejects a host rpath resolution', async () => {
  const fixture = await nativeBundleFixture('@rpath/libcodec.dylib')
  const hostLibrary = join(dirname(fixture.app), 'host-libs')
  await mkdir(hostLibrary)
  await writeFile(join(hostLibrary, 'libcodec.dylib'), 'host')
  await writeFile(
    join(fixture.toolDirectory, 'otool'),
    `#!/bin/bash
if [[ "$1" == "-L" ]]; then
  printf '%s:\\n\\t@rpath/libcodec.dylib (compatibility version 1.0.0, current version 1.0.0)\\n' "$2"
else
  printf 'Load command 1\\n          cmd LC_RPATH\\n      cmdsize 32\\n         path ${hostLibrary} (offset 12)\\n'
fi
`,
  )
  await chmod(join(fixture.toolDirectory, 'otool'), 0o755)
  const result = run(bundleVerifier, [fixture.app], {
    PATH: `${fixture.toolDirectory}:${process.env.PATH}`,
    VIEWER_VIDEO_RUNTIME_VERIFIER: fixture.verifier,
  })

  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /prohibited rpath/)
})

test('bundle verifier rejects same-basename rpath without a matching dylib install ID', async () => {
  const fixture = await nativeBundleFixture('@rpath/ffprobe')
  await writeFile(
    join(fixture.toolDirectory, 'otool'),
    `#!/bin/bash
if [[ "$1" == "-L" ]]; then
  if [[ "$2" == */bin/ffprobe ]]; then
    printf '%s:\\n\\t@rpath/ffprobe (compatibility version 1.0.0, current version 1.0.0)\\n' "$2"
  else
    printf '%s:\\n\\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)\\n' "$2"
  fi
elif [[ "$1" == "-D" ]]; then
  exit 1
else
  exit 0
fi
`,
  )
  await chmod(join(fixture.toolDirectory, 'otool'), 0o755)
  const result = run(bundleVerifier, [fixture.app], {
    PATH: `${fixture.toolDirectory}:${process.env.PATH}`,
    VIEWER_VIDEO_RUNTIME_VERIFIER: fixture.verifier,
  })

  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /unresolved rpath dependency/)
})

test('license verifier accepts the reviewed lock and complete notices', () => {
  const result = run(licenseVerifier)
  assert.equal(result.status, 0, result.stderr)
})

test('license verifier rejects an omitted runtime component', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-notices-'))
  const notices = join(directory, 'THIRD_PARTY_NOTICES.md')
  const source = await readFile(join(repository, 'THIRD_PARTY_NOTICES.md'), 'utf8')
  const mutated = source.replace(
    'ee21092a5ee427353392360929dc64645c54479aefdb5babc5cfbb5fad626209',
    'omitted-digest',
  )
  assert.notEqual(mutated, source, 'the fixture must remove the mpv archive digest')
  await writeFile(notices, mutated)

  const result = run(licenseVerifier, [], { VIEWER_VIDEO_NOTICES_FILE: notices })

  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /mpv.*notices/)
})

test('CI keeps pull requests offline and gates both release architectures', async () => {
  const workflow = await readFile(join(repository, '.github/workflows/ci.yml'), 'utf8')

  assert.match(workflow, /^  schedule:/m)
  assert.match(workflow, /^  release:/m)
  assert.match(workflow, /^  runtime:/m)
  assert.match(workflow, /github\.event_name == 'schedule'/)
  assert.match(workflow, /github\.event_name == 'release'/)
  assert.match(workflow, /aarch64-apple-darwin/)
  assert.match(workflow, /x86_64-apple-darwin/)
  assert.match(workflow, /scripts\/video\/build-macos-runtime\.sh/)
  assert.match(workflow, /scripts\/video\/stage-tauri-resources\.sh/)
  assert.match(workflow, /scripts\/video\/verify-app-bundle\.sh/)

  const pullRequestSection = workflow.match(/^  pull_request:\s*$([\s\S]*?)^jobs:/m)?.[0] ?? ''
  assert.doesNotMatch(pullRequestSection, /build-macos-runtime/)
})

test('development Tauri commands do not require release signing credentials', async () => {
  const [packageText, tauriText] = await Promise.all([
    readFile(join(repository, 'package.json'), 'utf8'),
    readFile(join(repository, 'src-tauri/tauri.conf.json'), 'utf8'),
  ])
  const packageJson = JSON.parse(packageText)
  const tauri = JSON.parse(tauriText)

  assert.equal(packageJson.scripts.tauri, 'tauri')
  assert.doesNotMatch(tauri.build.beforeDevCommand, /video:runtime:stage/)
  assert.match(tauri.build.beforeBuildCommand, /video:runtime:stage/)
})
