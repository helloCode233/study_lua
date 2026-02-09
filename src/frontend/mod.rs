//! Lua 前端入口模块。
//!
//! 编译流水线：
//! 1. `lexer`：源码字符串 -> token 列表
//! 2. `parser`：token 列表 -> AST（递归下降 + Pratt）
//! 3. `codegen`：AST -> `Vec<Proto>`（可直接交给 VM 执行）
//!
//! 对外只暴露两个主入口：
//! - `compile_str`：直接编译源码字符串
//! - `compile_file`：读取文件并编译

pub mod error;
pub mod token;

pub use crate::frontend::error::CompileError;
use std::path::Path;

/// 编译源码字符串到 Proto 集合。
///
/// 返回值约定：
/// - `Ok(Vec<Proto>)`：`protos[0]` 为主 chunk（可作为入口函数调用）
/// - `Err(CompileError)`：含阶段信息和定位信息
pub fn compile_str(src: &str) -> Result<Vec<crate::proto::Proto>, CompileError> {
    // let tokens = lexer::Lexer::new(src).tokenize()?;
    // let ast = parser::parse_tokens(tokens)?;
    // compile_ast(&ast)
    todo!()
}

/// 从文件读取脚本并编译。
///
/// 这里把 I/O 错误统一包装成 `CompileError::Io`，保持前端 API 的单一错误出口。
pub fn compile_file(path: &Path) -> Result<Vec<crate::proto::Proto>, CompileError> {
    let src = std::fs::read_to_string(path).map_err(|e| CompileError::Io(e.to_string()))?;
    compile_str(&src)
}
