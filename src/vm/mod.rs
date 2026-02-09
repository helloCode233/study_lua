//! VM 模块（Lua 风格、寄存器映射到统一值栈）。
//!
//! 这里的 VM 采用 Lua 5.x 类似的调用/栈模型：
//! - `stack` 同时承载：函数槽位、参数区、寄存器区、返回值写回区
//! - 一次调用从 `func_index` 开始：
//!   - `stack[func_index]` 是函数对象（当前只支持 `Value::LFn(proto_id)`）
//!   - 参数从 `stack[func_index + 1]` 起连续摆放
//!   - callee 的寄存器窗口 `base = func_index + 1`，即 R0 对应第一个参数槽位
//! - `Vm.top` 是“有效 top”（指向第一个空槽位），按 Lua 规则在 CALL/RETURN 时更新：
//!   - B=0 / C=0 的变参/多返回依赖它
//!   - 即使 B!=0 / C!=0（固定参数/固定返回），也要更新它，避免旧值污染后续 B=0/Return B=0
//! - 变参函数（`...`）：
//!   - `Proto.num_params` 描述固定参数个数
//!   - `Proto.is_vararg` 为 true 时，调用时传入的“多余参数”会被保存到 `CallFrame.varargs`
//!   - callee 通过 `VARARG` 指令把 varargs 按需拷贝进寄存器
//!
//! 文件拆分：
//! - `model.rs`：核心数据结构（Vm/CallFrame/ResultsSpec）
//! - `execute.rs`：字节码解释执行（run/run_results）
//! - `call.rs`：call/pcall 以及回滚/兜底 panic 捕获
//! - `stack.rs`：寄存器/栈访问与 top 维护（含 `set_top`）
//! - `frame.rs`：frame 与 checkpoint/rollback
//! - `common.rs`：小工具（类型检查、统一的 OOB 错误）
pub mod call;
pub mod common;
pub mod execute;
pub mod frame;
pub mod model;
pub mod stack;

// 对外只暴露 VM 类型，隐藏内部模块结构（tests/示例可直接 `use study_lua::vm::Vm;`）。
pub use model::Vm;
