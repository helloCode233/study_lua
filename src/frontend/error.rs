use std::fmt::{Display, Formatter};

/// 前端编译错误（从源码到 Proto 的每个阶段都统一走这个错误类型）。
///
/// 设计目标：
/// - 对学生友好：尽量带上 `line/col`，定位到具体源码位置
/// - 分阶段清晰：词法/语法/语义（codegen）错误分开
/// - 与 I/O 错误兼容：读取脚本文件失败也可以直接上抛
#[derive(Debug)]
pub enum CompileError {
    /// 词法阶段错误（非法字符、非法数字等）。
    Lex {
        line: usize,
        col: usize,
        msg: String,
    },
    /// 语法阶段错误（期望某个 token，但读到了另一个 token）。
    Parse {
        line: usize,
        col: usize,
        expected: String,
        found: String,
    },
    /// 当前教学阶段明确“不支持”的语法/语义。
    ///
    /// 例如：全局 `function foo()`、全局变量读写等。
    Unsupported {
        line: usize,
        col: usize,
        feature: String,
    },
    /// 代码生成阶段错误（AST -> Proto 过程中发现不一致/越界等）。
    Codegen {
        line: usize,
        col: usize,
        msg: String,
    },
    /// 文件读取等 I/O 错误。
    Io(String),
}

impl Display for CompileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::Lex { line, col, msg } => {
                write!(f, "lex error at {}:{}: {}", line, col, msg)
            }
            CompileError::Parse {
                line,
                col,
                expected,
                found,
            } => write!(
                f,
                "parse error at {}:{}: expected {}, found {}",
                line, col, expected, found
            ),
            CompileError::Unsupported { line, col, feature } => {
                write!(f, "unsupported feature at {}:{}: {}", line, col, feature)
            }
            CompileError::Codegen { line, col, msg } => {
                write!(f, "codegen error at {}:{}: {}", line, col, msg)
            }
            CompileError::Io(msg) => write!(f, "io error: {}", msg),
        }
    }
}

impl std::error::Error for CompileError {}
