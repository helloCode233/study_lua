use crate::Value;
use crate::proto::Proto;

#[derive(Copy, Clone, Debug)]
pub(crate) enum ResultsSpec {
    /// 固定返回值个数（0 表示丢弃返回值）。
    Fixed(usize),
    /// 多返回（Lua: Call 的 C=0 / Return 的 B=0 场景会用到 top 来决定数量）。
    Multi,
}

pub struct CallFrame {
    pub proto_id: Option<usize>,
    pub pc: isize,
    /// 栈上的函数槽位绝对索引：`stack[func]` 为被调函数对象。
    pub func: usize,
    /// 本帧寄存器基址：`R0` 映射到 `stack[base]`（Lua 风格：base = func + 1）。
    pub base: usize,
    /// 预留寄存器上界：`base + proto.max_stack`。
    /// 当前实现用它来扩容栈，后续 open-call/多返回会更依赖它。
    pub top: usize,
    /// 变参列表（来自 caller 传入的“多余参数”）。
    ///
    /// Lua 5.x 中，“多余参数”不会直接作为寄存器可见，而是通过 `VARARG` 指令按需拷贝进寄存器。
    pub varargs: Vec<Value>,
    /// 返回值规格：
    /// - Fixed(n): 固定写回 n 个（n=0 表示丢弃）
    /// - Multi: 多返回，写回后会更新 caller.top
    pub(crate) results: ResultsSpec,
}

/// 一个非常简化的 Lua 虚拟机（寄存器窗口映射到一段连续栈空间）。
pub struct Vm {
    /// 统一的值栈：同时承载函数槽位、参数区、寄存器区与返回值写回区。
    pub stack: Vec<Value>,
    /// 原型表（函数字节码 + 常量表）。
    pub protos: Vec<Proto>,
    /// 调用栈帧（frames[0] 是 sentinel；Lua 帧从 1 开始）。
    pub frames: Vec<CallFrame>,
    /// “有效 top”（Lua 风格）：指向栈上第一个空槽位。
    ///
    /// 这个 top 主要服务于 Lua 5.x 的“变参/多返回”语义（B=0 / C=0），但为了保持行为一致，
    /// 我们也会在固定参数/固定返回（B!=0 / C!=0）时按 Lua 规则更新它：
    /// - Call B=0：参数来自 R[A+1]..R[top-1]
    /// - Return B=0：返回值来自 R[A]..R[top-1]
    /// - Call C=0：写回后会更新 top
    /// - Call B!=0：进入 callee 前把 top 设为参数末尾（top = ra + B）
    /// - Call C!=0：返回后把 top 设为返回值末尾（top = ra + (C-1)）
    ///
    /// 注意：`stack.len()` 更像容量/可访问区间，不能简单用 truncate 同步为 top。
    pub top: usize,
}

impl Vm {
    pub fn new(protos: Vec<Proto>) -> Vm {
        let sentinel = CallFrame {
            proto_id: None,
            pc: 0,
            func: 0,
            base: 0,
            top: 0,
            varargs: vec![],
            results: ResultsSpec::Fixed(0),
        };
        Self {
            stack: Vec::new(),
            frames: vec![sentinel],
            protos,
            top: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub(crate) struct VmCheckpoint {
    pub(crate) top: usize,
    pub(crate) frames_len: usize,
}
