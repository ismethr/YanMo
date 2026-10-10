use std::path::{Path, PathBuf};
use ym_latex::{index_project, parse_file, FlowItem};
use ym_model::{AssetKind, SectionLevel};

fn parse(src: &str) -> ym_latex::FileIndex {
    parse_file(Path::new("t.tex"), src)
}

fn names(v: &[ym_latex::Located]) -> Vec<&str> {
    v.iter().map(|l| l.name.as_str()).collect()
}

#[test]
fn headings_and_labels() {
    let f = parse(
        "\\section{Intro}\\label{sec:intro}\nSome text.\n\\subsection*[S]{Detail \\textbf{x}}\n% \\section{Hidden}\nMore \\label{loose}",
    );
    assert_eq!(f.headings.len(), 2);
    assert_eq!(f.headings[0].title, "Intro");
    assert_eq!(f.headings[0].label.as_deref(), Some("sec:intro"));
    assert_eq!(f.headings[0].span.line, 1);
    assert_eq!(f.headings[1].level, SectionLevel::Subsection);
    assert!(f.headings[1].starred);
    assert_eq!(f.headings[1].title, "Detail x");
    assert_eq!(
        f.headings[1].label, None,
        "label after text must not attach"
    );
    assert_eq!(names(&f.labels), ["sec:intro", "loose"]);
}

#[test]
fn refs_and_cites() {
    let f = parse(
        r"See \ref{a}, \cref{b, c} and \crefrange{d}{e}. \citep[p.~3]{k1,k2} \parencites{k3}[see][]{k4} \nocite{*} \citestyle{plain}",
    );
    assert_eq!(names(&f.refs), ["a", "b", "c", "d", "e"]);
    assert_eq!(names(&f.cites), ["k1", "k2", "k3", "k4"]);
    assert!(f.nocite_all);
    // key 的位置精确到字符
    let k2 = &f.cites[1];
    assert_eq!(
        &r"See \ref{a}, \cref{b, c} and \crefrange{d}{e}. \citep[p.~3]{k1,k2}"
            [k2.span.start..k2.span.end],
        "k2"
    );
}

#[test]
fn assets() {
    let f = parse(
        r"\begin{figure}[t]
  \centering
  \includegraphics[width=.5\linewidth]{figures/a}
  \caption{An \emph{arch} figure\label{fig:a}}
\end{figure}
\begin{table}\caption{Res}\label{tab:r}\begin{tabular}{lc}x&y\end{tabular}\end{table}
\begin{equation}\label{eq:1} y=f(x) \end{equation}
\begin{equation} z \end{equation}
\begin{align*} w \end{align*}",
    );
    assert_eq!(f.assets.len(), 3, "{:#?}", f.assets);
    let fig = &f.assets[0];
    assert_eq!(fig.kind, AssetKind::Figure);
    assert_eq!(fig.label.as_deref(), Some("fig:a"));
    assert_eq!(fig.caption.as_deref(), Some("An arch figure"));
    assert_eq!(fig.files, [PathBuf::from("figures/a")]);
    assert_eq!(f.assets[1].kind, AssetKind::Table);
    assert_eq!(f.assets[1].label.as_deref(), Some("tab:r"));
    assert_eq!(f.assets[2].kind, AssetKind::Equation);
}

#[test]
fn skips_verbatim_comments_and_iffalse() {
    let f = parse(
        "\\verb|\\label{v1}| 50\\% \\label{ok} % \\label{c}\n\\begin{verbatim}\n\\label{v2}\n\\end{verbatim}\n\\iffalse \\label{v3} \\fi \\label{ok2}",
    );
    assert_eq!(names(&f.labels), ["ok", "ok2"]);
}

#[test]
fn word_count_excludes_preamble_and_math() {
    let f = parse(
        "\\documentclass{article}\n\\title{Not counted}\n\\begin{document}\nOne two $x + y$ three.\n\\begin{equation} a b c \\end{equation}\n中文四字\n\\end{document}\nafter end",
    );
    assert!(f.is_root_document);
    let total: u32 = f
        .flow
        .iter()
        .map(|i| match i {
            FlowItem::Words(n) => *n,
            _ => 0,
        })
        .sum();
    assert_eq!(total, 3 + 4);
}

#[test]
fn incomplete_source_does_not_hang_or_panic() {
    // 模拟"写到一半"：在每个字符边界截断都必须能解析
    let src = "\\documentclass{ctexart}\\begin{document}\n\\section{引言}\\label{sec:i}\n文字 $x^2 \\cite{a,b}\n\n\\begin{figure}\\caption{图\\label{fig:x}}\\includegraphics{f.png}\\end{figure}\n\\verb|x| \\input{sec} \\ref{fig:x}\n\\end{document}";
    for (i, _) in src.char_indices() {
        let _ = parse(&src[..i]);
    }
    let f = parse(src);
    assert_eq!(names(&f.refs), ["fig:x"]);
    assert_eq!(names(&f.cites), ["a", "b"]);
    assert_eq!(names(&f.includes), ["sec"]);
}

#[test]
fn unterminated_argument_stops_at_paragraph() {
    let f = parse("\\section{Unclosed\n\nBody text \\label{later}");
    assert!(f.headings.is_empty());
    assert_eq!(names(&f.labels), ["later"]);
}

#[test]
fn sample_paper_project() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample-paper");
    let main = ym_latex::find_main(&root).expect("main.tex");
    assert_eq!(main, Path::new("main.tex"));
    let p = index_project(&root, &main).unwrap();

    assert_eq!(p.files.len(), 3);
    assert!(p.unresolved_includes.is_empty());
    let titles: Vec<_> = p.outline.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["引言", "方法"]);
    assert_eq!(p.outline[0].id, "sec:intro");
    assert!(p.outline[0].words > 10);
    assert_eq!(p.outline[1].span.file, Path::new("sections/method.tex"));

    let fig = p.assets().find(|a| a.kind == AssetKind::Figure).unwrap();
    assert_eq!(fig.label.as_deref(), Some("fig:arch"));
    assert!(fig.references.is_empty());
    assert_eq!(names(&p.files[0].bib_files), ["refs"]);
}

#[test]
fn main_detection_in_multi_document_folder() {
    // 模拟期刊投稿目录：正文、补充材料、标题页、亮点都是独立文档
    let dir = std::env::temp_dir().join(format!("ym-roots-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let doc = |body: &str| {
        format!("\\documentclass{{article}}\n\\begin{{document}}\n{body}\n\\end{{document}}\n")
    };
    std::fs::write(dir.join("Highlights.tex"), doc("short")).unwrap();
    std::fs::write(dir.join("title_page.tex"), doc("title")).unwrap();
    std::fs::write(
        dir.join("supplement.tex"),
        doc(&"long supplement text ".repeat(200)),
    )
    .unwrap();
    std::fs::write(dir.join("manuscript.tex"), doc("\\section{Intro} text")).unwrap();
    std::fs::write(dir.join("notes.tex"), "% 不是独立文档\n").unwrap();

    let roots = ym_latex::find_roots(&dir);
    assert_eq!(roots.len(), 4, "{roots:?}");
    assert_eq!(
        ym_latex::find_main(&dir).unwrap(),
        PathBuf::from("manuscript.tex")
    );

    // 没有常见主文件名时取篇幅最长、且不像附属文档的那个
    std::fs::remove_file(dir.join("manuscript.tex")).unwrap();
    std::fs::write(dir.join("chapter-draft.tex"), doc(&"draft ".repeat(50))).unwrap();
    assert_eq!(
        ym_latex::find_main(&dir).unwrap(),
        PathBuf::from("chapter-draft.tex")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
