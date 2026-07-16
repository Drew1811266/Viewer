import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), 'utf8')

test('repository verification inputs are exact and locked', async () => {
  const [workflow, toolchain, packageText] = await Promise.all([
    read('.github/workflows/ci.yml'),
    read('rust-toolchain.toml'),
    read('package.json'),
  ])
  const packageJson = JSON.parse(packageText)

  assert.match(workflow, /runs-on: macos-15/)
  assert.match(workflow, /actions\/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0/)
  assert.match(workflow, /actions\/setup-node@820762786026740c76f36085b0efc47a31fe5020/)
  assert.match(workflow, /node-version: "24\.18\.0"/)
  assert.match(workflow, /rustup toolchain install 1\.97\.0/)
  assert.match(workflow, /cargo deny --locked check/)
  assert.doesNotMatch(workflow, /actions\/(checkout|setup-node)@v\d/)

  assert.match(toolchain, /channel = "1\.97\.0"/)
  assert.match(packageJson.scripts.verify, /^node --test scripts\/repository-policy\.test\.mjs && /)
  assert.match(packageJson.scripts.verify, /cargo clippy --locked /)
  assert.match(packageJson.scripts.verify, /cargo test --locked --workspace$/)
})
