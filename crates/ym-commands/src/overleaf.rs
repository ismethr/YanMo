//! 与 Overleaf 互通：把项目源码打包成可直接上传到 Overleaf 的 zip，或把 Overleaf 下载的 zip 解压为本地项目。

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ym_model::tr;
use zip::write::SimpleFileOptions;

use crate::CommandError;

/// 不放进 zip 的编译中间文件
const SKIP_EXT: &[&str] = &[
    "aux",
    "log",
    "out",
    "toc",
    "lof",
    "lot",
    "blg",
    "fls",
    "fdb_latexmk",
    "synctex",
    "xdv",
    "bcf",
    "run.xml",
    "nav",
    "snm",
    "vrb",
];

fn skip(rel: &Path) -> bool {
    let name = rel
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    // 隐藏文件与目录（.yanmo、.git、.DS_Store……）
    if rel
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        return true;
    }
    name.ends_with(".synctex.gz") || SKIP_EXT.iter().any(|e| name.ends_with(&format!(".{e}")))
}

/// Overleaf 用不上、体积又大的文件：Office 文档、压缩包
const OMIT_EXT: &[&str] = &[
    "docx", "doc", "xlsx", "xls", "pptx", "ppt", "rtf", "odt", "zip", "7z", "rar", "gz", "tgz",
    "bz2", "xz", "dmg", "exe",
];

#[derive(Debug, Default, serde::Serialize)]
pub struct Omitted {
    /// 与同名 .tex 放在一起的 PDF，多半是编译产物
    pub compiled_pdf: Vec<String>,
    /// Office 文档、压缩包
    pub other: Vec<String>,
}

fn walk(
    root: &Path,
    rel: &Path,
    out: &mut Vec<PathBuf>,
    omitted: &mut Omitted,
) -> std::io::Result<()> {
    for e in fs::read_dir(root.join(rel))? {
        let e = e?;
        let p = rel.join(e.file_name());
        if skip(&p) {
            continue;
        }
        let ty = e.file_type()?;
        if ty.is_dir() {
            walk(root, &p, out, omitted)?;
            continue;
        }
        if !ty.is_file() {
            continue;
        }
        let ext = p
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let name = p.to_string_lossy().replace('\\', "/");
        if OMIT_EXT.contains(&ext.as_str()) {
            omitted.other.push(name);
        } else if ext == "pdf" && root.join(p.with_extension("tex")).is_file() {
            omitted.compiled_pdf.push(name);
        } else {
            out.push(p);
        }
    }
    Ok(())
}

/// 主文件是否需要 XeLaTeX（Overleaf 默认用 pdfLaTeX）
pub fn needs_xelatex(main_source: &str) -> bool {
    let preamble = main_source
        .split("\\begin{document}")
        .next()
        .unwrap_or(main_source);
    let lower = preamble.to_ascii_lowercase();
    lower.contains("!tex program = xelatex")
        || [
            "ctexart",
            "ctexrep",
            "ctexbook",
            "ctexbeamer",
            "{ctex}",
            "xecjk",
            "fontspec",
        ]
        .iter()
        .any(|k| lower.contains(k))
}

pub struct Exported {
    pub files: usize,
    pub bytes: u64,
    pub omitted: Omitted,
    /// 是否补了让 Overleaf 用 XeLaTeX 编译的 latexmkrc
    pub added_latexmkrc: bool,
}

/// Overleaf 用 latexmkrc 决定编译引擎：XeLaTeX 为 5，LuaLaTeX 为 4
pub fn latexmkrc_for(engine: Option<ym_model::Engine>, main_source: &str) -> Option<&'static str> {
    match engine {
        Some(ym_model::Engine::Lualatex) => {
            Some("# YanMo: compile with LuaLaTeX on Overleaf\n$pdf_mode = 4;\n")
        }
        Some(ym_model::Engine::Pdflatex) => None,
        Some(ym_model::Engine::Xelatex) => Some(XELATEX_RC),
        _ if needs_xelatex(main_source) => Some(XELATEX_RC),
        _ => None,
    }
}

const XELATEX_RC: &str = "# YanMo: compile with XeLaTeX on Overleaf\n$pdf_mode = 5;\n";

/// 打包项目源码。项目里没有 latexmkrc 时按需补一个，让 Overleaf 用正确的引擎编译。
pub fn export_zip(
    root: &Path,
    out: &Path,
    latexmkrc: Option<&str>,
) -> Result<Exported, CommandError> {
    let mut files = Vec::new();
    let mut omitted = Omitted::default();
    walk(root, Path::new(""), &mut files, &mut omitted)?;
    files.sort();
    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir)?;
    }
    let err = |e: zip::result::ZipError| CommandError::Failed(e.to_string());
    let mut zip = zip::ZipWriter::new(File::create(out)?);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut bytes = 0;
    for rel in &files {
        let name = rel.to_string_lossy().replace('\\', "/");
        zip.start_file(name, opts).map_err(err)?;
        let data = fs::read(root.join(rel))?;
        bytes += data.len() as u64;
        zip.write_all(&data)?;
    }
    let has_rc = files.iter().any(|f| {
        let n = f.to_string_lossy();
        n == "latexmkrc" || n == ".latexmkrc"
    });
    let added_latexmkrc = latexmkrc.is_some() && !has_rc;
    if let (Some(rc), true) = (latexmkrc, added_latexmkrc) {
        zip.start_file("latexmkrc", opts).map_err(err)?;
        zip.write_all(rc.as_bytes())?;
    }
    zip.finish().map_err(err)?;
    Ok(Exported {
        files: files.len() + usize::from(added_latexmkrc),
        bytes,
        omitted,
        added_latexmkrc,
    })
}

/// 解压 zip 到 `dest`（必须不存在）。只有一个顶层目录时去掉这一层；拒绝越出目标目录的路径。
pub fn import_zip(zip_path: &Path, dest: &Path) -> Result<usize, CommandError> {
    if dest.exists() {
        return Err(CommandError::Failed(tr!(
            "{p} 已存在，请换一个名称",
            "{p} already exists; choose another name",
            p = dest.display()
        )));
    }
    let err = |e: zip::result::ZipError| {
        CommandError::Failed(tr!("无法读取 zip：{e}", "Cannot read the zip: {e}"))
    };
    let mut zip = zip::ZipArchive::new(File::open(zip_path)?).map_err(err)?;
    let mut entries = Vec::new();
    for i in 0..zip.len() {
        let f = zip.by_index(i).map_err(err)?;
        let Some(name) = f.enclosed_name() else {
            return Err(CommandError::Failed(tr!(
                "zip 中有不安全的路径：{n}",
                "The zip contains an unsafe path: {n}",
                n = f.name()
            )));
        };
        // macOS 压缩时附带的元数据
        if name.starts_with("__MACOSX") || name.file_name().is_some_and(|n| n == ".DS_Store") {
            continue;
        }
        entries.push((i, name, f.is_dir()));
    }
    // 所有条目都在同一个顶层目录下时去掉它
    let first = |p: &PathBuf| p.components().next().map(|c| c.as_os_str().to_owned());
    let strip = match entries.first().and_then(|(_, p, _)| first(p)) {
        Some(top)
            if entries.iter().all(|(_, p, d)| {
                first(p).as_ref() == Some(&top) && (p.components().count() > 1 || *d)
            }) =>
        {
            Some(PathBuf::from(top))
        }
        _ => None,
    };
    fs::create_dir_all(dest)?;
    let mut count = 0;
    for (i, name, is_dir) in entries {
        let rel = match &strip {
            Some(top) => name.strip_prefix(top).unwrap_or(&name).to_path_buf(),
            None => name,
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let path = dest.join(&rel);
        if is_dir {
            fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(d) = path.parent() {
            fs::create_dir_all(d)?;
        }
        let mut f = zip.by_index(i).map_err(err)?;
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        fs::write(&path, data)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ym-overleaf-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn round_trip_skips_build_and_hidden_files() {
        let src = tmp("src");
        fs::create_dir_all(src.join("sections")).unwrap();
        fs::create_dir_all(src.join(".yanmo/build")).unwrap();
        fs::create_dir_all(src.join("figures")).unwrap();
        fs::write(
            src.join("main.tex"),
            "\\documentclass{ctexart}\n\\begin{document}\n\\input{sections/a}\n\\end{document}\n",
        )
        .unwrap();
        fs::write(src.join("sections/a.tex"), "中文").unwrap();
        fs::write(src.join("figures/f.pdf"), b"%PDF").unwrap();
        fs::write(src.join("main.aux"), "x").unwrap();
        fs::write(src.join(".yanmo/build/main.pdf"), "x").unwrap();
        fs::write(src.join(".DS_Store"), "x").unwrap();
        fs::write(src.join("main.pdf"), "x").unwrap();
        fs::write(src.join("main.docx"), "x").unwrap();

        let out = tmp("out").join("p.zip");
        let main = fs::read_to_string(src.join("main.tex")).unwrap();
        let r = export_zip(&src, &out, latexmkrc_for(None, &main)).unwrap();
        assert!(r.added_latexmkrc);
        assert_eq!(r.files, 4); // main.tex, sections/a.tex, figures/f.pdf, latexmkrc
        assert_eq!(r.omitted.compiled_pdf, ["main.pdf"]);
        assert_eq!(r.omitted.other, ["main.docx"]);

        let dest = tmp("dest");
        assert_eq!(import_zip(&out, &dest).unwrap(), 4);
        assert_eq!(
            fs::read_to_string(dest.join("sections/a.tex")).unwrap(),
            "中文"
        );
        assert!(dest.join("latexmkrc").is_file());
        assert!(!dest.join("main.aux").exists());
        // 目标已存在时拒绝
        assert!(import_zip(&out, &dest).is_err());
        for d in [src, out.parent().unwrap().to_path_buf(), dest] {
            let _ = fs::remove_dir_all(d);
        }
    }

    #[test]
    fn strips_single_top_folder_and_rejects_unsafe_paths() {
        let zpath = tmp("z").with_extension("zip");
        {
            let mut z = zip::ZipWriter::new(File::create(&zpath).unwrap());
            let o = SimpleFileOptions::default();
            z.start_file("paper/main.tex", o).unwrap();
            z.write_all(b"x").unwrap();
            z.start_file("paper/sec/a.tex", o).unwrap();
            z.write_all(b"y").unwrap();
            z.start_file("__MACOSX/paper/._main.tex", o).unwrap();
            z.write_all(b"junk").unwrap();
            z.finish().unwrap();
        }
        let dest = tmp("strip");
        assert_eq!(import_zip(&zpath, &dest).unwrap(), 2);
        assert!(dest.join("main.tex").is_file() && dest.join("sec/a.tex").is_file());

        let evil = tmp("evil").with_extension("zip");
        {
            let mut z = zip::ZipWriter::new(File::create(&evil).unwrap());
            z.start_file("../escape.tex", SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        let dest2 = tmp("evil-dest");
        assert!(import_zip(&evil, &dest2).is_err());
        assert!(!dest2.parent().unwrap().join("escape.tex").exists());
        for p in [zpath, evil, dest, dest2] {
            let _ = fs::remove_dir_all(&p);
            let _ = fs::remove_file(&p);
        }
    }

    #[test]
    fn detects_xelatex_documents() {
        assert!(needs_xelatex("\\documentclass{ctexart}\n\\begin{document}"));
        assert!(needs_xelatex(
            "% !TEX program = xelatex\n\\documentclass{article}"
        ));
        assert!(!needs_xelatex(
            "\\documentclass{article}\n\\begin{document} fontspec in body"
        ));
    }
}
