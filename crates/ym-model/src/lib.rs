//! 项目与文档模型。
//!
//! 原则：tex 源码是唯一事实来源，这里的结构都由解析得到；
//! 只有用户元数据（章节状态、目标字数等）持久化在 `.yanmo/project.toml`。

pub mod asset;
pub mod bib;
pub mod outline;
pub mod project;
pub mod span;

pub use asset::{Asset, AssetKind};
pub use bib::BibEntry;
pub use outline::{Section, SectionLevel, SectionStatus};
pub use project::{Engine, Project, ProjectMeta};
pub use span::SourceSpan;
