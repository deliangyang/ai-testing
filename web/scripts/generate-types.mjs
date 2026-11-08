import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { compile } from 'json-schema-to-typescript'

const webRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), '..')
const repoRoot = path.join(webRoot, '..')
const outDir = path.join(webRoot, 'src/types/generated')
const check = process.argv.includes('--check')

const entries = [
  { stem: 'dag', schemaFile: 'dag.schema.json' },
  { stem: 'run-report', schemaFile: 'run-report.schema.json' },
]

function writeOrCheck(outPath, content) {
  if (check) {
    let onDisk
    try {
      onDisk = readFileSync(outPath, 'utf8')
    } catch {
      console.error(`missing generated file: ${outPath}`)
      process.exit(1)
    }
    if (onDisk !== content) {
      console.error(
        `stale ${path.relative(webRoot, outPath)}; regenerate: cd web && npm run generate:types`,
      )
      process.exit(1)
    }
    return
  }
  writeFileSync(outPath, content)
}

mkdirSync(outDir, { recursive: true })

for (const { stem, schemaFile } of entries) {
  const schemaPath = path.join(repoRoot, 'schemas', schemaFile)
  const schema = JSON.parse(readFileSync(schemaPath, 'utf8'))
  const banner = `/**\n * Generated from \`schemas/${schemaFile}\`. Do not edit.\n * @see npm run generate:types\n */\n`
  const ts = await compile(schema, stem, {
    bannerComment: banner,
    additionalProperties: false,
    enableConstEnums: true,
    strictIndexSignatures: true,
  })
  writeOrCheck(path.join(outDir, `${stem}.ts`), ts)
}

const indexBody = `${entries.map((e) => `export * from './${e.stem}'`).join('\n')}\n`
const indexBanner = '/** Generated barrel — sync via `npm run generate:types`. */\n'
writeOrCheck(path.join(outDir, 'index.ts'), indexBanner + indexBody)

if (check) {
  console.log('generated types are up to date')
}
