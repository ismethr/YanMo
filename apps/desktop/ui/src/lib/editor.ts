// CodeMirror 6 配置：LaTeX 高亮、浅色主题（读取 CSS 变量）、问题面板同源的波浪线诊断。

import { basicSetup } from 'codemirror';
import { EditorView } from '@codemirror/view';
import { Compartment, type EditorState, type Extension } from '@codemirror/state';
import { HighlightStyle, StreamLanguage, syntaxHighlighting } from '@codemirror/language';
import { stex } from '@codemirror/legacy-modes/mode/stex';
import type { Diagnostic } from '@codemirror/lint';
import { tags as t } from '@lezer/highlight';
import type { Issue } from './types';

const latex = StreamLanguage.define(stex);

const theme = EditorView.theme({
  '&': { height: '100%', fontSize: 'var(--editor-font-size, 13.5px)', color: 'var(--fg)', backgroundColor: 'var(--bg)' },
  '.cm-scroller': { fontFamily: 'var(--mono)', lineHeight: '1.65' },
  '.cm-content': { caretColor: 'var(--fg)', padding: '8px 0' },
  '.cm-gutters': { backgroundColor: 'var(--bg)', color: 'var(--fg3)', border: 'none' },
  '.cm-activeLine': { backgroundColor: 'var(--cur)' },
  '.cm-activeLineGutter': { backgroundColor: 'var(--cur)', color: 'var(--fg)' },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': { background: 'var(--sel)' },
  '.cm-cursor': { borderLeftColor: 'var(--fg)' },
  '.cm-tooltip': { background: 'var(--bg2)', border: '1px solid var(--line)' },
});

const highlight = HighlightStyle.define([
  { tag: [t.tagName, t.keyword], color: 'var(--tk-cmd)' },
  { tag: [t.atom, t.typeName], color: 'var(--tk-env)' },
  { tag: [t.string, t.special(t.string)], color: 'var(--tk-str)' },
  { tag: t.number, color: 'var(--tk-cite)' },
  { tag: t.comment, color: 'var(--tk-com)', fontStyle: 'italic' },
  { tag: [t.bracket, t.punctuation], color: 'var(--fg2)' },
  { tag: t.invalid, color: 'var(--err)' },
]);

/** 自动换行可在设置中切换；同一个 Compartment 用于所有文件的编辑器状态 */
export const wrapping = new Compartment();
export const wrapExtension = (on: boolean): Extension => (on ? EditorView.lineWrapping : []);

export function extensionsFor(path: string, listener: Extension, wrap = true): Extension[] {
  return [
    basicSetup,
    wrapping.of(wrapExtension(wrap)),
    path.endsWith('.tex') || path.endsWith('.sty') || path.endsWith('.cls') ? latex : [],
    theme,
    syntaxHighlighting(highlight),
    listener,
  ];
}

/** 把 ym check 的问题转为 CodeMirror 诊断：优先标出出错的 label / key / 路径，找不到时标整行。 */
export function diagnosticsFor(state: EditorState, issues: Issue[]): Diagnostic[] {
  const out: Diagnostic[] = [];
  for (const issue of issues) {
    if (!issue.line || issue.line > state.doc.lines) continue;
    const line = state.doc.line(issue.line);
    const token = issue.label ?? issue.key ?? issue.path;
    const at = token ? line.text.indexOf(token) : -1;
    const lead = line.text.length - line.text.trimStart().length;
    const from = at >= 0 ? line.from + at : line.from + lead;
    const to = at >= 0 ? from + token!.length : line.to;
    out.push({
      from,
      to: Math.max(to, from),
      severity: issue.severity,
      message: issue.message,
      source: issue.code,
    });
  }
  return out;
}
