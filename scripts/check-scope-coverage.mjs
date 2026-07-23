#!/usr/bin/env node

import { readFileSync } from 'node:fs'
import { validateScopeCoverage } from './scope-coverage.mjs'

const result = validateScopeCoverage(
  readFileSync('docs/PRODUCT_SPEC.md', 'utf8'),
  readFileSync('docs/milestones/viewer-0.1-scope-matrix.md', 'utf8'),
)

console.log(
  `Viewer scope coverage passed: ${result.requirementCount} requirements mapped exactly once; ${result.m3Count} frozen M3 requirements`,
)
