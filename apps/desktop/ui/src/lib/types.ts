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
  span: SourceSpan;
  words: number;
  children: Section[];
}

export interface Asset {
  kind: 'figure' | 'table' | 'equation' | 'algorithm' | 'listing';
  label: string | null;
  caption: string | null;
  files: string[];
  span: SourceSpan;
  references: SourceSpan[];
}

export interface ProjectInfo {
  root: string;
  name: string;
  main: string | null;
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
  citations: SourceSpan[];
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
  mutating: boolean;
}
