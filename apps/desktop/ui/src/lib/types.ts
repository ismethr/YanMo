// 与 Rust 侧 ym-commands 返回的 JSON 结构一一对应。

export interface SourceSpan {
  file: string;
  start: number;
  end: number;
  line: number;
}

export interface Section {
  id: string;
  level: 'part' | 'chapter' | 'section' | 'subsection' | 'subsubsection' | 'paragraph';
  title: string;
  label: string | null;
  /** \section* 等不编号的为 false */
  numbered: boolean;
  span: SourceSpan;
  words: number;
  children: Section[];
}

export interface Asset {
  kind: 'figure' | 'table' | 'equation' | 'algorithm' | 'listing';
  label: string | null;
  caption: string | null;
  files: string[];
  /** files 对应的项目内路径（找不到时为 null） */
  paths?: (string | null)[];
  span: SourceSpan;
  references: SourceSpan[];
}

export interface ProjectInfo {
  root: string;
  name: string;
  main: string | null;
  /** 目录中所有独立文档（含 \documentclass），相对项目根目录 */
  roots: string[];
}

export interface TreeNode {
  name: string;
  path: string;
  kind: 'dir' | 'tex' | 'bib' | 'image' | 'other';
  children?: TreeNode[];
}

export interface ProjectIndex {
  main: string;
  total_words: number;
  front_words: number;
  outline: Section[];
  assets: Asset[];
  labels: string[];
  files: string[];
}

export interface Issue {
  code: string;
  severity: 'error' | 'warning';
  message: string;
  file: string | null;
  line: number | null;
  label?: string;
  key?: string;
  path?: string;
}

export interface BibEntry {
  key: string;
  entry_type: string;
  fields: Record<string, string>;
  span: SourceSpan;
  /** 本文档中的引用位置 */
  citations: SourceSpan[];
  /** 没有被任何文档引用（考虑了补充材料等同目录文档） */
  unused?: boolean;
}

export interface CheckResult {
  errors: number;
  warnings: number;
  issues: Issue[];
  bib: BibEntry[];
}

export interface CommandMeta {
  id: string;
  title: string;
  title_en: string;
  mutating: boolean;
}

export interface CompileDiagnostic {
  severity: 'error' | 'warning' | 'badbox';
  file: string | null;
  line: number | null;
  message: string;
}

export interface CompileOutput {
  success: boolean;
  engine: 'tectonic' | 'xelatex' | 'pdflatex' | 'lualatex';
  pdf: string | null;
  synctex: string | null;
  diagnostics: CompileDiagnostic[];
  duration_ms: number;
  log_tail: string;
  /** 被停止或超时 */
  stopped: boolean;
  /** 完整日志，相对项目根目录 */
  log: string | null;
}

/** 项目设置（.yanmo/project.toml） */
export interface ProjectSettings {
  engine: 'auto' | 'tectonic' | 'xelatex' | 'pdflatex' | 'lualatex';
  shell_escape: boolean;
  main: string | null;
}

export interface Toolchain {
  tectonic: string | null;
  latexmk: string | null;
  pandoc: string | null;
  ghostscript: string | null;
}

export interface ExportOutput {
  path: string;
  duration_ms: number;
  numbered_refs: boolean;
  labels: number;
  figures_converted: number;
  figures_missing: string[];
  csl: string | null;
  reference_doc: string | null;
  warnings: string[];
}

/** PDF 坐标（点），原点在页面左上角 */
export interface PdfRect {
  page: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface SourceLocation {
  file: string;
  line: number;
}

/** 文献核查结果（bib.verify） */
export interface BibVerification {
  key: string;
  status: 'ok' | 'mismatch' | 'resolved' | 'not_found' | 'skipped' | 'error';
  notes: string[];
  found: { title: string; authors: string[]; year: number | null; venue: string | null; doi: string | null; source: string } | null;
  suggested_doi: string | null;
}

/** Zotero 文献库中的条目（zotero.search） */
export interface ZoteroItem {
  key: string;
  title: string;
  authors: string;
  year: string | null;
  item_type: string;
  doi: string | null;
}
