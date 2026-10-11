use std::fs;
use std::path::{Path, PathBuf};
use ym_latex::index_project;
use ym_latex::reorder::{plan_move, MoveError, MovePlan, Position};

/// 在临时目录里建一个项目：files 为 (相对路径, 内容)
fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ym-reorder-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for (rel, text) in files {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    dir
}

fn plan(dir: &Path, id: &str, target: &str, pos: Position) -> Result<MovePlan, MoveError> {
    let index = index_project(dir, Path::new("main.tex")).unwrap();
    plan_move(&index, id, target, pos)
}

const SINGLE: &str = r"\documentclass{article}
\begin{document}
\section{A}\label{sec:a}
Alpha text.

\section{B}\label{sec:b}
Beta text.
\subsection{B1}\label{sec:b1}
B one.
\subsection{B2}\label{sec:b2}
B two.

\section{C}\label{sec:c}
Gamma text.

\bibliographystyle{plain}
\bibliography{refs}
\end{document}
";

#[test]
fn single_file_move_last_section_first() {
    let dir = project("single1", &[("main.tex", SINGLE)]);
    let p = plan(&dir, "sec:c", "sec:a", Position::Before).unwrap();
    assert_eq!(p.file, Path::new("main.tex"));
    let expected = r"\documentclass{article}
\begin{document}
\section{C}\label{sec:c}
Gamma text.

\section{A}\label{sec:a}
Alpha text.

\section{B}\label{sec:b}
Beta text.
\subsection{B1}\label{sec:b1}
B one.
\subsection{B2}\label{sec:b2}
B two.

\bibliographystyle{plain}
\bibliography{refs}
\end{document}
";
    assert_eq!(p.new_text, expected);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn single_file_move_section_with_children_to_end() {
    let dir = project("single2", &[("main.tex", SINGLE)]);
    let p = plan(&dir, "sec:b", "sec:c", Position::After).unwrap();
    // 子节随父节一起移动，参考文献留在原处
    let body = &p.new_text;
    let c = body.find(r"\section{C}").unwrap();
    let b = body.find(r"\section{B}").unwrap();
    let b2 = body.find(r"\subsection{B2}").unwrap();
    let bib = body.find(r"\bibliographystyle").unwrap();
    assert!(c < b && b < b2 && b2 < bib);
    // 只是换位置：内容总量不变
    assert_eq!(body.len(), SINGLE.len());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn subsections_swap_within_parent() {
    let dir = project("sub", &[("main.tex", SINGLE)]);
    let p = plan(&dir, "sec:b2", "sec:b1", Position::Before).unwrap();
    assert!(p
        .new_text
        .contains("\\subsection{B2}\\label{sec:b2}\nB two.\n\n\\subsection{B1}"));
    // 最后一个子节的范围止于下一个 \section 前（含空行），移动后 B1 紧接 C
    assert!(p.new_text.contains("B one.\n\\section{C}"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rejects_level_mismatch_and_moving_into_itself() {
    let dir = project("bad", &[("main.tex", SINGLE)]);
    assert!(matches!(
        plan(&dir, "sec:a", "sec:b1", Position::Before),
        Err(MoveError::LevelMismatch)
    ));
    assert!(matches!(
        plan(&dir, "sec:x", "sec:a", Position::Before),
        Err(MoveError::NotFound(_))
    ));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn no_op_when_already_in_place() {
    let dir = project("noop", &[("main.tex", SINGLE)]);
    let p = plan(&dir, "sec:a", "sec:b", Position::Before).unwrap();
    assert_eq!(p.new_text, p.old_text);
    let _ = fs::remove_dir_all(dir);
}

const SPLIT_MAIN: &str = r"\documentclass{article}
\begin{document}
\input{sections/intro}
\input{sections/method}
\input{sections/results}
\bibliography{refs}
\end{document}
";

#[test]
fn section_per_file_moves_input_lines() {
    let dir = project(
        "split",
        &[
            ("main.tex", SPLIT_MAIN),
            (
                "sections/intro.tex",
                "% 引言\n\\section{Intro}\\label{sec:intro}\nI.\n",
            ),
            (
                "sections/method.tex",
                "\\section{Method}\\label{sec:method}\nM.\n\\subsection{Data}\nD.\n",
            ),
            (
                "sections/results.tex",
                "\\section{Results}\\label{sec:results}\nR.\n",
            ),
        ],
    );
    let p = plan(&dir, "sec:results", "sec:intro", Position::Before).unwrap();
    assert_eq!(p.file, Path::new("main.tex"));
    assert_eq!(
        p.new_text,
        "\\documentclass{article}\n\\begin{document}\n\\input{sections/results}\n\\input{sections/intro}\n\\input{sections/method}\n\\bibliography{refs}\n\\end{document}\n"
    );
    let p = plan(&dir, "sec:intro", "sec:results", Position::After).unwrap();
    assert!(p.new_text.contains("\\input{sections/method}\n\\input{sections/results}\n\\input{sections/intro}\n\\bibliography"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn mixed_inline_and_included_sections() {
    let main = "\\documentclass{article}\n\\begin{document}\n\\section{Inline}\\label{sec:inline}\nText.\n\n\\input{b}\n\\end{document}\n";
    let dir = project(
        "mixed",
        &[
            ("main.tex", main),
            ("b.tex", "\\section{B}\\label{sec:b}\nB.\n"),
        ],
    );
    let p = plan(&dir, "sec:b", "sec:inline", Position::Before).unwrap();
    assert_eq!(
        p.new_text,
        "\\documentclass{article}\n\\begin{document}\n\\input{b}\n\\section{Inline}\\label{sec:inline}\nText.\n\n\\end{document}\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn moves_into_a_file_with_several_sections() {
    // a.tex 有两节；b.tex 一节。把 B 移到 A1 之前：在 main 中调整 \input 顺序
    let main =
        "\\documentclass{article}\n\\begin{document}\n\\input{a}\n\\input{b}\n\\end{document}\n";
    let dir = project(
        "multi",
        &[
            ("main.tex", main),
            (
                "a.tex",
                "\\section{A1}\\label{sec:a1}\nx\n\\section{A2}\\label{sec:a2}\ny\n",
            ),
            ("b.tex", "\\section{B}\\label{sec:b}\nz\n"),
        ],
    );
    let p = plan(&dir, "sec:b", "sec:a1", Position::Before).unwrap();
    assert_eq!(p.file, Path::new("main.tex"));
    assert!(p.new_text.contains("\\input{b}\n\\input{a}\n"));
    // 把 A1 移到 A2 之后：在 a.tex 内完成
    let p = plan(&dir, "sec:a1", "sec:a2", Position::After).unwrap();
    assert_eq!(p.file, Path::new("a.tex"));
    assert_eq!(
        p.new_text,
        "\\section{A2}\\label{sec:a2}\ny\n\\section{A1}\\label{sec:a1}\nx\n"
    );
    // B 插到 A2 之前需要拆开 a.tex，不支持
    assert!(matches!(
        plan(&dir, "sec:b", "sec:a2", Position::Before),
        Err(MoveError::CrossFile)
    ));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn file_without_trailing_newline() {
    let main = "\\documentclass{article}\n\\begin{document}\n\\section{A}\\label{sec:a}\na\n\\section{B}\\label{sec:b}\nb";
    let dir = project("eof", &[("main.tex", main)]);
    let p = plan(&dir, "sec:a", "sec:b", Position::After).unwrap();
    assert_eq!(
        p.new_text,
        "\\documentclass{article}\n\\begin{document}\n\\section{B}\\label{sec:b}\nb\n\\section{A}\\label{sec:a}\na\n"
    );
    let _ = fs::remove_dir_all(dir);
}

/// 对项目中每一对同级兄弟章节做"之前/之后"两种移动，检查：
/// 只重排文本（行的集合不变）；重新解析后大纲中该节紧挨目标，其余章节相对顺序不变。
fn check_all_moves(dir: &Path, main: &str) -> usize {
    use ym_model::Section;
    let index = index_project(dir, Path::new(main)).unwrap();
    fn siblings(list: &[Section], out: &mut Vec<Vec<Section>>) {
        out.push(list.to_vec());
        for s in list {
            siblings(&s.children, out);
        }
    }
    fn order(list: &[Section]) -> Vec<String> {
        list.iter().map(|s| s.id.clone()).collect()
    }
    fn find_list<'a>(list: &'a [Section], first: &str) -> Option<&'a [Section]> {
        if list.iter().any(|s| s.id == first) {
            return Some(list);
        }
        list.iter().find_map(|s| find_list(&s.children, first))
    }
    let mut groups = Vec::new();
    siblings(&index.outline, &mut groups);
    let mut checked = 0;
    for group in groups.iter().filter(|g| g.len() > 1) {
        for s in group {
            for t in group {
                if s.id == t.id {
                    continue;
                }
                for pos in [Position::Before, Position::After] {
                    let plan = match plan_move(&index, &s.id, &t.id, pos) {
                        Ok(p) => p,
                        Err(MoveError::CrossFile) => continue,
                        Err(e) => panic!("{} → {}: {e}", s.id, t.id),
                    };
                    let mut a: Vec<&str> = plan.old_text.lines().collect();
                    let mut b: Vec<&str> = plan.new_text.lines().collect();
                    a.sort();
                    b.sort();
                    assert_eq!(a, b, "{} {:?} {}: 行集合变了", s.id, pos, t.id);

                    // 写到副本里重新解析
                    let copy = dir.with_extension("moved");
                    let _ = fs::remove_dir_all(&copy);
                    copy_dir(dir, &copy);
                    fs::write(copy.join(&plan.file), &plan.new_text).unwrap();
                    let after = index_project(&copy, Path::new(main)).unwrap();
                    let list =
                        find_list(&after.outline, &s.id).expect("moved section still in outline");
                    let new_order = order(list);
                    let mut expected: Vec<String> =
                        order(group).into_iter().filter(|x| *x != s.id).collect();
                    let ti = expected.iter().position(|x| *x == t.id).unwrap();
                    expected.insert(
                        if pos == Position::Before { ti } else { ti + 1 },
                        s.id.clone(),
                    );
                    assert_eq!(new_order, expected, "{} {:?} {}", s.id, pos, t.id);
                    let _ = fs::remove_dir_all(&copy);
                    checked += 1;
                }
            }
        }
    }
    checked
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

#[test]
fn every_sibling_move_only_reorders() {
    let dir = project("all-single", &[("main.tex", SINGLE)]);
    assert!(check_all_moves(&dir, "main.tex") > 10);
    let _ = fs::remove_dir_all(dir);
    let dir = project(
        "all-split",
        &[
            ("main.tex", SPLIT_MAIN),
            ("sections/intro.tex", "\\section{Intro}\\label{sec:intro}\nI.\n"),
            ("sections/method.tex", "\\section{Method}\\label{sec:method}\nM.\n\\subsection{Data}\nD.\n\\subsection{Model}\nE.\n"),
            ("sections/results.tex", "\\section{Results}\\label{sec:results}\nR.\n"),
        ],
    );
    assert!(check_all_moves(&dir, "main.tex") > 10);
    let _ = fs::remove_dir_all(dir);
}

/// 用真实论文跑一遍：YM_REORDER_DIR=<论文副本目录> YM_REORDER_MAIN=<主文件> cargo test -- --ignored
#[test]
#[ignore = "需要指定论文目录"]
fn every_sibling_move_in_real_paper() {
    let dir = PathBuf::from(std::env::var("YM_REORDER_DIR").unwrap());
    let main = std::env::var("YM_REORDER_MAIN").unwrap();
    let n = check_all_moves(&dir, &main);
    eprintln!("checked {n} moves");
}

use ym_latex::reorder::plan_extract;

fn outline_signature(dir: &Path, main: &str) -> Vec<String> {
    fn walk(list: &[ym_model::Section], depth: usize, out: &mut Vec<String>) {
        for s in list {
            out.push(format!(
                "{depth}|{}|{}|{:?}|{}",
                s.id, s.title, s.level, s.words
            ));
            walk(&s.children, depth + 1, out);
        }
    }
    let index = index_project(dir, Path::new(main)).unwrap();
    let mut out = vec![format!("front|{}", index.front_words)];
    walk(&index.outline, 0, &mut out);
    out
}

/// 把计划写进项目目录
fn apply_extract(dir: &Path, p: &ym_latex::reorder::ExtractPlan) {
    fs::write(dir.join(&p.parent), &p.parent_new).unwrap();
    let f = dir.join(&p.file);
    fs::create_dir_all(f.parent().unwrap()).unwrap();
    fs::write(f, &p.text).unwrap();
}

#[test]
fn extract_section_from_single_file() {
    let dir = project("extract1", &[("main.tex", SINGLE)]);
    let before = outline_signature(&dir, "main.tex");
    let index = index_project(&dir, Path::new("main.tex")).unwrap();
    let p = plan_extract(&index, "sec:b").unwrap();
    assert_eq!(p.parent, Path::new("main.tex"));
    assert_eq!(p.file, Path::new("sections/b.tex"));
    assert_eq!(
        p.text,
        "\\section{B}\\label{sec:b}\nBeta text.\n\\subsection{B1}\\label{sec:b1}\nB one.\n\\subsection{B2}\\label{sec:b2}\nB two.\n"
    );
    assert!(p
        .parent_new
        .contains("Alpha text.\n\n\\input{sections/b}\n\n\\section{C}"));
    apply_extract(&dir, &p);
    assert_eq!(outline_signature(&dir, "main.tex"), before);

    // 已经是独立文件的不能再拆
    let index = index_project(&dir, Path::new("main.tex")).unwrap();
    assert!(matches!(
        plan_extract(&index, "sec:b"),
        Err(MoveError::AlreadyOwnFile)
    ));
    // 拆出的子节放进同一个目录，文件名不冲突
    let p = plan_extract(&index, "sec:b1").unwrap();
    assert_eq!(p.parent, Path::new("sections/b.tex"));
    assert_eq!(p.file, Path::new("sections/b1.tex"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn extract_names_without_label_or_ascii_title() {
    let main = "\\documentclass{ctexart}\n\\begin{document}\n\\section{引言}\n甲。\n\\section{方法}\n乙。\n\\end{document}\n";
    let dir = project(
        "extract-cn",
        &[("main.tex", main), ("sections/section-2.tex", "% 已存在\n")],
    );
    let index = index_project(&dir, Path::new("main.tex")).unwrap();
    let id = index.outline[1].id.clone();
    let p = plan_extract(&index, &id).unwrap();
    assert_eq!(p.file, Path::new("sections/section-2-2.tex"));
    assert!(p
        .parent_new
        .ends_with("甲。\n\\input{sections/section-2-2}\n\\end{document}\n"));
    let _ = fs::remove_dir_all(dir);
}

/// 对每一节分别拆分：拆分后重新解析的大纲（层级、标题、字数）与拆分前完全一致
fn check_all_extracts(dir: &Path, main: &str) -> usize {
    let before = outline_signature(dir, main);
    let index = index_project(dir, Path::new(main)).unwrap();
    let mut ids = Vec::new();
    fn collect(list: &[ym_model::Section], out: &mut Vec<String>) {
        for s in list {
            out.push(s.id.clone());
            collect(&s.children, out);
        }
    }
    collect(&index.outline, &mut ids);
    let mut n = 0;
    for id in ids {
        let p = match plan_extract(&index, &id) {
            Ok(p) => p,
            Err(MoveError::AlreadyOwnFile | MoveError::CrossFile) => continue,
            Err(e) => panic!("{id}: {e}"),
        };
        let copy = dir.with_extension("extracted");
        let _ = fs::remove_dir_all(&copy);
        copy_dir(dir, &copy);
        apply_extract(&copy, &p);
        assert_eq!(
            outline_signature(&copy, main),
            before,
            "拆分 {id} 后大纲变了"
        );
        let _ = fs::remove_dir_all(&copy);
        n += 1;
    }
    n
}

#[test]
fn every_extract_keeps_outline() {
    let dir = project("extract-all", &[("main.tex", SINGLE)]);
    assert_eq!(check_all_extracts(&dir, "main.tex"), 5);
    let _ = fs::remove_dir_all(dir);
}

#[test]
#[ignore = "需要指定论文目录"]
fn every_extract_in_real_paper() {
    let dir = PathBuf::from(std::env::var("YM_REORDER_DIR").unwrap());
    let main = std::env::var("YM_REORDER_MAIN").unwrap();
    eprintln!("checked {} extracts", check_all_extracts(&dir, &main));
}
