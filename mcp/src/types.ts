/** Mirrors the service's `PageType` enum. */
export type PageType = 'text' | 'ocr' | 'mixed'

/** Mirrors `PageContent` in src/models.rs. */
export interface PageContent {
  page: number
  type: PageType
  content: string
  confidence?: number
}

/** Mirrors `DocumentJson` in src/models.rs. */
export interface DocumentJson {
  pages?: PageContent[]
  has_ocr: boolean
  ocr_page_indices?: number[]
  format: string
  page_count?: number
  ocr_confidence?: number[]
}

/** Mirrors `ConvertResult` in src/models.rs — one per file, streamed as NDJSON. */
export interface ConvertResult {
  index: number
  filename: string
  markdown?: string
  json?: DocumentJson
  error?: string
  elapsed_ms: number
}

/** Mirrors `HealthResponse`. */
export interface HealthResponse {
  status: string
  version: string
  ocr_available: boolean
}
