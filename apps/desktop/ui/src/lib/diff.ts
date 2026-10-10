// 词级 diff：英文按单词、中文按字切分，LCS 求最长公共子序列。用于展示 AI 改写前后的差异。

export interface DiffPart {
  kind: 'same' | 'add' | 'del';
  text: string;
}

function tokens(s: string): string[] {
  return s.match(/\s+|[A-Za-z0-9_\\]+|[一-鿿]|[^\sA-Za-z0-9_\\一-鿿]/g) ?? [];
}

/** 超过此规模时退化为整段替换，避免 O(n·m) 过慢 */
const MAX_CELLS = 2_000_000;

export function wordDiff(before: string, after: string): DiffPart[] {
  const a = tokens(before);
  const b = tokens(after);
  if (a.length * b.length > MAX_CELLS) {
    return [
      { kind: 'del', text: before },
      { kind: 'add', text: after },
    ];
  }
  // dp[i][j] = a[i..] 与 b[j..] 的 LCS 长度
  const w = b.length + 1;
  const dp = new Uint32Array((a.length + 1) * w);
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      dp[i * w + j] = a[i] === b[j] ? dp[(i + 1) * w + j + 1] + 1 : Math.max(dp[(i + 1) * w + j], dp[i * w + j + 1]);
    }
  }
  const out: DiffPart[] = [];
  const push = (kind: DiffPart['kind'], text: string) => {
    const last = out[out.length - 1];
    if (last && last.kind === kind) last.text += text;
    else out.push({ kind, text });
  };
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      push('same', a[i]);
      i++;
      j++;
    } else if (dp[(i + 1) * w + j] >= dp[i * w + j + 1]) {
      push('del', a[i++]);
    } else {
      push('add', b[j++]);
    }
  }
  while (i < a.length) push('del', a[i++]);
  while (j < b.length) push('add', b[j++]);
  return out;
}
