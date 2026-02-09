use crate::opcode::BITRK;

pub mod opcode;
pub mod proto;
pub mod vm;

#[derive(Clone, Debug)]
pub enum Value {
    Number(f64),
    LFn(usize),
    Bool(bool),
    Nil,
}

/// VM 运行时错误（用于替代 `panic!`，让错误能被 `pcall` 捕获并回滚状态）。
#[derive(Debug, Clone)]
pub enum VmError {
    /// 试图在没有 Lua 帧的情况下执行 `run()`。
    NoLuaFrame,
    /// `pc` 越界：当前指令地址不在 `code` 范围内。
    PcOutOfBounds { pc: usize, code_len: usize },
    /// 未知 opcode。
    UnknownOpcode(u32),
    /// 保留：历史上用于拒绝 B=0/C=0（变参/多返回）模式；当前实现已支持，一般不会再返回该错误。
    UnsupportedCall { b: u32, c: u32 },
    /// 被调用值不是可调用对象（当前只支持 `Value::LFn`）。
    NotCallable(Value),
    /// 访问原型表越界。
    ProtoOutOfBounds { index: usize, len: usize },
    /// 类型错误（例如把 Bool 当 Number）。
    TypeError { expected: &'static str, got: Value },
    /// 任意栈/常量等索引越界（替代 `Vec[idx]` 的 panic）。
    StackOutOfBounds { index: usize, len: usize },
    /// 兜底：捕获到了意外 `panic`（通常表示 VM 实现 bug）。
    Panic(String),
}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmError::NoLuaFrame => write!(f, "no lua vm: call pcall/call first"),
            VmError::PcOutOfBounds { pc, code_len } => {
                write!(f, "pc out of bounds: pc={}, code_len={}", pc, code_len)
            }
            VmError::UnknownOpcode(op) => write!(f, "unknown opcode: {}", op),
            VmError::UnsupportedCall { b, c } => write!(
                f,
                "unsupported call mode: B={} (nargs+1), C={} (nresults+1)",
                b, c
            ),
            VmError::NotCallable(v) => write!(f, "value is not callable: {:?}", v),
            VmError::ProtoOutOfBounds { index, len } => {
                write!(f, "proto out of bounds: index={}, len={}", index, len)
            }
            VmError::TypeError { expected, got } => {
                write!(f, "type error: expected {}, got {:?}", expected, got)
            }
            VmError::StackOutOfBounds { index, len } => {
                write!(f, "index out of bounds: index={}, len={}", index, len)
            }
            VmError::Panic(msg) => write!(f, "panic: {}", msg),
        }
    }
}

impl std::error::Error for VmError {}

#[derive(Clone, Debug)]
pub enum Operand {
    R(usize), // register index
    K(usize), // const index
}
// 0 2 4 8 16
// (1+2+4+8)+1 = 16
pub fn rk_r(r: u32) -> u32 {
    r
}
pub fn rk_k(k: u32) -> u32 {
    k | BITRK
}
