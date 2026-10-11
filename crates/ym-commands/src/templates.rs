//! 新建论文的模板。每个模板都按研墨推荐的结构组织：`main.tex` + `sections/` 下每节一个文件 +
//! `refs.bib` + `figures/`，用内置 Tectonic 即可直接编译（宏包按需下载）。

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Template {
    pub id: &'static str,
    pub name: &'static str,
    pub name_en: &'static str,
    pub description: &'static str,
    pub description_en: &'static str,
    /// (相对路径, 内容)；内容中的 `{{title}}` 会替换为论文标题
    #[serde(skip)]
    pub files: &'static [(&'static str, &'static str)],
}

const REFS_EN: &str = r#"@inproceedings{vaswani2017attention,
  author    = {Vaswani, Ashish and Shazeer, Noam and Parmar, Niki and Uszkoreit, Jakob and Jones, Llion and Gomez, Aidan N. and Kaiser, {\L}ukasz and Polosukhin, Illia},
  title     = {Attention Is All You Need},
  booktitle = {Advances in Neural Information Processing Systems},
  year      = {2017},
  volume    = {30},
}

@inproceedings{he2016deep,
  author    = {He, Kaiming and Zhang, Xiangyu and Ren, Shaoqing and Sun, Jian},
  title     = {Deep Residual Learning for Image Recognition},
  booktitle = {Proceedings of the IEEE Conference on Computer Vision and Pattern Recognition},
  year      = {2016},
  pages     = {770--778},
  doi       = {10.1109/CVPR.2016.90},
}
"#;

const INTRO_EN: &str = r"\section{Introduction}\label{sec:intro}
State the problem, why it matters, and what this paper contributes.
Deep residual networks~\cite{he2016deep} and attention~\cite{vaswani2017attention} are examples of how to cite.
Section~\ref{sec:method} describes the method and Section~\ref{sec:results} reports the results.
";

const METHOD_EN: &str = r"\section{Method}\label{sec:method}
Describe the data and the method. Number important equations and refer to them, as in Eq.~\eqref{eq:model}:
\begin{equation}\label{eq:model}
  y = f(x) + \varepsilon .
\end{equation}
";

const RESULTS_EN: &str = r"\section{Results}\label{sec:results}
Table~\ref{tab:results} summarizes the main results. Put figures in the figures/ folder and include them with \verb|\includegraphics|.
\begin{table}[t]
  \centering
  \caption{Main results.}\label{tab:results}
  \begin{tabular}{lcc}
    \hline
    Method & Accuracy & Time (s) \\
    \hline
    Baseline & 0.81 & 12 \\
    Ours     & 0.87 & 10 \\
    \hline
  \end{tabular}
\end{table}
";

const CONCLUSION_EN: &str = r"\section{Conclusion}\label{sec:conclusion}
Summarize the findings, their limits, and future work.
";

const KEEP: &str = "";

const ARTICLE_EN: &[(&str, &str)] = &[
    (
        "main.tex",
        r"\documentclass[11pt]{article}
\usepackage[margin=2.5cm]{geometry}
\usepackage{amsmath,graphicx}
\usepackage[numbers]{natbib}
\usepackage{hyperref}

\title{{{title}}}
\author{Author Name\\ Affiliation\\ \texttt{author@example.com}}
\date{}

\begin{document}
\maketitle

\begin{abstract}
One paragraph summarizing the question, the method, the main result and why it matters.
\end{abstract}

\input{sections/introduction}
\input{sections/method}
\input{sections/results}
\input{sections/conclusion}

\bibliographystyle{plainnat}
\bibliography{refs}
\end{document}
",
    ),
    ("sections/introduction.tex", INTRO_EN),
    ("sections/method.tex", METHOD_EN),
    ("sections/results.tex", RESULTS_EN),
    ("sections/conclusion.tex", CONCLUSION_EN),
    ("refs.bib", REFS_EN),
    ("figures/.gitkeep", KEEP),
];

const ELSARTICLE: &[(&str, &str)] = &[
    (
        "main.tex",
        r"\documentclass[preprint,12pt,authoryear]{elsarticle}
\usepackage{amsmath,graphicx}

\journal{Journal Name}

\begin{document}
\begin{frontmatter}
\title{{{title}}}

\author[a]{First Author\corref{cor1}}
\ead{first.author@example.com}
\author[a]{Second Author}
\cortext[cor1]{Corresponding author.}
\affiliation[a]{organization={Department, University},
  city={City}, country={Country}}

\begin{abstract}
One paragraph summarizing the question, the method, the main result and why it matters.
\end{abstract}

\begin{keyword}
keyword one \sep keyword two \sep keyword three
\end{keyword}
\end{frontmatter}

\input{sections/introduction}
\input{sections/method}
\input{sections/results}
\input{sections/conclusion}

\bibliographystyle{elsarticle-harv}
\bibliography{refs}
\end{document}
",
    ),
    ("sections/introduction.tex", INTRO_EN),
    ("sections/method.tex", METHOD_EN),
    ("sections/results.tex", RESULTS_EN),
    ("sections/conclusion.tex", CONCLUSION_EN),
    ("refs.bib", REFS_EN),
    ("figures/.gitkeep", KEEP),
];

const IEEE: &[(&str, &str)] = &[
    (
        "main.tex",
        r"\documentclass[conference]{IEEEtran}
\usepackage{amsmath,graphicx,cite}

\begin{document}
\title{{{title}}}

\author{\IEEEauthorblockN{First Author}
\IEEEauthorblockA{Department, University\\ City, Country\\ first.author@example.com}
\and
\IEEEauthorblockN{Second Author}
\IEEEauthorblockA{Department, University\\ City, Country\\ second.author@example.com}}

\maketitle

\begin{abstract}
One paragraph summarizing the question, the method, the main result and why it matters.
\end{abstract}

\begin{IEEEkeywords}
keyword one, keyword two, keyword three
\end{IEEEkeywords}

\input{sections/introduction}
\input{sections/method}
\input{sections/results}
\input{sections/conclusion}

\bibliographystyle{IEEEtran}
\bibliography{refs}
\end{document}
",
    ),
    ("sections/introduction.tex", INTRO_EN),
    ("sections/method.tex", METHOD_EN),
    ("sections/results.tex", RESULTS_EN),
    ("sections/conclusion.tex", CONCLUSION_EN),
    ("refs.bib", REFS_EN),
    ("figures/.gitkeep", KEEP),
];

const ACM: &[(&str, &str)] = &[
    (
        "main.tex",
        r"\documentclass[sigconf]{acmart}

\begin{document}
\title{{{title}}}

\author{First Author}
\affiliation{%
  \institution{University}
  \city{City}
  \country{Country}}
\email{first.author@example.com}

\author{Second Author}
\affiliation{%
  \institution{University}
  \city{City}
  \country{Country}}
\email{second.author@example.com}

\begin{abstract}
One paragraph summarizing the question, the method, the main result and why it matters.
\end{abstract}

\keywords{keyword one, keyword two, keyword three}

\maketitle

\input{sections/introduction}
\input{sections/method}
\input{sections/results}
\input{sections/conclusion}

\bibliographystyle{ACM-Reference-Format}
\bibliography{refs}
\end{document}
",
    ),
    ("sections/introduction.tex", INTRO_EN),
    ("sections/method.tex", METHOD_EN),
    ("sections/results.tex", RESULTS_EN),
    ("sections/conclusion.tex", CONCLUSION_EN),
    ("refs.bib", REFS_EN),
    ("figures/.gitkeep", KEEP),
];

const LLNCS: &[(&str, &str)] = &[
    (
        "main.tex",
        r"\documentclass[runningheads]{llncs}
\usepackage{amsmath,graphicx}
\usepackage{hyperref}

\begin{document}
\title{{{title}}}
\author{First Author\inst{1} \and Second Author\inst{1}}
\authorrunning{F. Author and S. Author}
\institute{University, City, Country\\
\email{first.author@example.com}}

\maketitle

\begin{abstract}
One paragraph summarizing the question, the method, the main result and why it matters.
\keywords{keyword one \and keyword two \and keyword three}
\end{abstract}

\input{sections/introduction}
\input{sections/method}
\input{sections/results}
\input{sections/conclusion}

\bibliographystyle{splncs04}
\bibliography{refs}
\end{document}
",
    ),
    ("sections/introduction.tex", INTRO_EN),
    ("sections/method.tex", METHOD_EN),
    ("sections/results.tex", RESULTS_EN),
    ("sections/conclusion.tex", CONCLUSION_EN),
    ("refs.bib", REFS_EN),
    ("figures/.gitkeep", KEEP),
];

const REFS_ZH: &str = r#"@article{zhang2020gengdi,
  author  = {张三 and 李四},
  title   = {耕地破碎化的时空演变及其驱动因素},
  journal = {地理学报},
  year    = {2020},
  volume  = {75},
  number  = {3},
  pages   = {500--512},
}

@inproceedings{he2016deep,
  author    = {He, Kaiming and Zhang, Xiangyu and Ren, Shaoqing and Sun, Jian},
  title     = {Deep Residual Learning for Image Recognition},
  booktitle = {Proceedings of the IEEE Conference on Computer Vision and Pattern Recognition},
  year      = {2016},
  pages     = {770--778},
  doi       = {10.1109/CVPR.2016.90},
}
"#;

const ARTICLE_ZH: &[(&str, &str)] = &[
    (
        "main.tex",
        r"\documentclass[12pt]{ctexart}
\usepackage[margin=2.5cm]{geometry}
\usepackage{amsmath,graphicx}
\usepackage{gbt7714}
\usepackage{hyperref}

\title{{{title}}}
\author{作者姓名\\ 单位\\ \texttt{author@example.com}}
\date{}

\begin{document}
\maketitle

\begin{abstract}
用一段话概括研究问题、方法、主要结果及其意义。
\end{abstract}

\input{sections/introduction}
\input{sections/method}
\input{sections/results}
\input{sections/conclusion}

\bibliography{refs}
\end{document}
",
    ),
    (
        "sections/introduction.tex",
        r"\section{引言}\label{sec:intro}
说明研究的问题、意义以及本文的贡献。引用文献的写法如~\cite{zhang2020gengdi,he2016deep}。
第~\ref{sec:method}~节介绍方法，第~\ref{sec:results}~节给出结果。
",
    ),
    (
        "sections/method.tex",
        r"\section{数据与方法}\label{sec:method}
介绍数据与方法。重要公式应编号并在正文中引用，如式~\eqref{eq:model}：
\begin{equation}\label{eq:model}
  y = f(x) + \varepsilon .
\end{equation}
",
    ),
    (
        "sections/results.tex",
        r"\section{结果}\label{sec:results}
表~\ref{tab:results}~汇总了主要结果。图片放在 figures 目录，用 \verb|\includegraphics| 插入。
\begin{table}[t]
  \centering
  \caption{主要结果}\label{tab:results}
  \begin{tabular}{lcc}
    \hline
    方法 & 精度 & 耗时（秒） \\
    \hline
    基线方法 & 0.81 & 12 \\
    本文方法 & 0.87 & 10 \\
    \hline
  \end{tabular}
\end{table}
",
    ),
    (
        "sections/conclusion.tex",
        r"\section{结论}\label{sec:conclusion}
总结主要发现、局限与后续工作。
",
    ),
    ("refs.bib", REFS_ZH),
    ("figures/.gitkeep", KEEP),
];

pub const TEMPLATES: &[Template] = &[
    Template {
        id: "article-zh",
        name: "中文论文",
        name_en: "Chinese article",
        description: "ctexart，参考文献按 GB/T 7714 著录",
        description_en: "ctexart with GB/T 7714 references",
        files: ARTICLE_ZH,
    },
    Template {
        id: "article-en",
        name: "英文论文",
        name_en: "Article",
        description: "通用的 article 文档类，natbib 引用",
        description_en: "The standard article class with natbib",
        files: ARTICLE_EN,
    },
    Template {
        id: "elsarticle",
        name: "Elsevier 期刊",
        name_en: "Elsevier journal",
        description: "elsarticle，作者-年份引用",
        description_en: "elsarticle with author-year citations",
        files: ELSARTICLE,
    },
    Template {
        id: "ieee",
        name: "IEEE 会议",
        name_en: "IEEE conference",
        description: "IEEEtran 双栏会议格式",
        description_en: "IEEEtran two-column conference format",
        files: IEEE,
    },
    Template {
        id: "acm",
        name: "ACM 会议",
        name_en: "ACM conference",
        description: "acmart sigconf",
        description_en: "acmart sigconf",
        files: ACM,
    },
    Template {
        id: "llncs",
        name: "Springer LNCS",
        name_en: "Springer LNCS",
        description: "Lecture Notes in Computer Science",
        description_en: "Lecture Notes in Computer Science",
        files: LLNCS,
    },
];

pub fn find(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.id == id)
}
