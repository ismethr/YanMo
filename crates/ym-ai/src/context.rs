//! 上下文组装：在 token 预算内挑选最相关的项目信息喂给模型。
//! 优先级：选中文本 > 当前章节 > 大纲 > 相关 label/caption > 相关文献元数据 > 编译诊断。

pub struct ContextBudget {
    pub max_tokens: usize,
}
