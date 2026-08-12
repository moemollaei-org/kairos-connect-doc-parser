#!/usr/bin/env node
/**
 * Smoke test: starts the server over stdio, lists its tools, and calls the one
 * tool that needs no input files. Deliberately touches no private documents so
 * it is safe to run anywhere.
 *
 *   DOC_PARSER_API_KEY=... node test/smoke.mjs
 */
import { Client } from '@modelcontextprotocol/sdk/client/index.js'
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

const here = dirname(fileURLToPath(import.meta.url))
const entry = resolve(here, '../dist/index.js')

const EXPECTED = [
  'doc_parser_health',
  'convert_documents',
  'convert_document_raw',
  'inspect_documents',
]

const client = new Client({ name: 'smoke', version: '1.0.0' })
await client.connect(
  new StdioClientTransport({ command: 'node', args: [entry], env: process.env }),
)

const { tools } = await client.listTools()
const names = tools.map((t) => t.name).sort()
const missing = EXPECTED.filter((n) => !names.includes(n))
if (missing.length) {
  console.error(`FAIL missing tools: ${missing.join(', ')}`)
  process.exit(1)
}
console.log(`ok   ${names.length} tools registered: ${names.join(', ')}`)

const health = await client.callTool({ name: 'doc_parser_health', arguments: {} })
const text = health.content[0].text
if (health.isError || !text.includes('status: ok')) {
  console.error(`FAIL health check:\n${text}`)
  process.exit(1)
}
console.log(`ok   health reachable\n${text.split('\n').map((l) => `       ${l}`).join('\n')}`)

await client.close()
console.log('\nall smoke checks passed')
