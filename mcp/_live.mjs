import { Client } from '@modelcontextprotocol/sdk/client/index.js'
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js'
const D = '/Users/moe/Library/Mobile Documents/com~apple~CloudDocs/Documents/moira_docs/registration documents'
const c = new Client({ name: 'live', version: '1.0.0' })
await c.connect(new StdioClientTransport({
  command: 'node', args: ['./dist/index.js'],
  env: { ...process.env, DOC_PARSER_BASE_URL: 'https://parse-doc.thekairos.app' },
}))
const t0 = Date.now()
console.log('--- 1. doc_parser_health ---')
console.log((await c.callTool({ name: 'doc_parser_health', arguments: {} })).content[0].text)

console.log('\n--- 2. inspect_documents: all 6 individually would exceed cap; use 3 ---')
const three = ['Moira-UBO-Registratie','Moira-Solution-Belastingplicht','Kairos-Connect-Belastingplicht'].map(n=>`${D}/${n}.pdf`)
let s = Date.now()
console.log((await c.callTool({ name: 'inspect_documents', arguments: { paths: three } })).content[0].text)
console.log(`   [wall ${((Date.now()-s)/1000).toFixed(1)}s]`)

console.log('\n--- 3. convert_documents (json mode, 1 file) ---')
s = Date.now()
const r = await c.callTool({ name: 'convert_documents', arguments: { paths: [`${D}/Kairos-KvK.pdf`], output: 'json', max_chars: 700 } })
console.log(r.content[0].text.slice(0, 900))
console.log(`   [wall ${((Date.now()-s)/1000).toFixed(1)}s]`)

console.log('\n--- 4. convert_document_raw (markdown) ---')
s = Date.now()
const raw = await c.callTool({ name: 'convert_document_raw', arguments: { path: `${D}/Moira-KvK.pdf`, output: 'markdown', max_chars: 400 } })
console.log(raw.content[0].text.slice(0, 700))
console.log(`   [wall ${((Date.now()-s)/1000).toFixed(1)}s]`)

console.log(`\nTOTAL MCP session: ${((Date.now()-t0)/1000).toFixed(1)}s`)
await c.close()
