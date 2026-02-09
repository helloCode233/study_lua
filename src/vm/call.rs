use crate::vm::model::ResultsSpec;
use crate::vm::model::{CallFrame, UpvalueRef, Vm};
use crate::{Value, VmError};
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

impl Vm {
    /// 将某个 `proto_id` 作为函数对象压栈，返回其“函数槽位索引”。
    /// 后续可用 `pcall(func_index, nargs, nresults)` 来调用它。
    pub fn load(&mut self, proto_id: usize) -> Result<usize, VmError> {
        if proto_id >= self.protos.len() {
            return Err(Self::proto_oob(proto_id, self.protos.len()));
        }
        let func = self.top;
        if self.stack.len() == self.top {
            self.stack.push(Value::LFn(proto_id));
        } else {
            self.stack[self.top] = Value::LFn(proto_id);
        }
        self.top += 1;
        Ok(func)
    }

    /// 建立一个新的 Lua 调用帧（不直接执行；执行由 `run()` 驱动）。
    ///
    /// 注意：`func_index` 是“栈索引”，不是 `proto_id`。
    pub fn call(
        &mut self,
        func_index: usize,
        nargs: usize,
        nresults: usize,
    ) -> Result<(), VmError> {
        self.call_with_results(func_index, nargs, ResultsSpec::Fixed(nresults))
    }

    pub(crate) fn call_with_results(
        &mut self,
        func_index: usize,
        nargs: usize,
        results: ResultsSpec,
    ) -> Result<(), VmError> {
        self.call_with_results_target(func_index, nargs, results, func_index)
    }

    /// 建立一个 Lua 调用帧，并允许指定“返回值写回起点”。
    ///
    /// 默认 CALL 会把返回值写回到函数槽位本身（`return_target == func_index`）。
    /// 但像 TFORCALL 这类指令需要把返回值写到其它寄存器区间（例如 `R(A+3)` 起），
    /// 因此这里单独抽象成可配置写回目标的入口。
    pub(crate) fn call_with_results_target(
        &mut self,
        func_index: usize,
        nargs: usize,
        results: ResultsSpec,
        return_target: usize,
    ) -> Result<(), VmError> {
        // 栈布局（Lua 风格）：
        // - stack[func_index] 是被调函数对象
        // - 参数区从 stack[func_index + 1] 开始，连续 nargs 个
        // - 本帧寄存器窗口 base = func_index + 1，因此 R0 对应第一个参数槽位
        //
        // Lua 5.x “top” 关键点（这里用 `Vm.top` 表示“有效 top”）：
        // - 进入 callee 前，需要把 caller.top 设为“参数末尾”（即第一个空槽位）。
        //   这样 B=0（变参）时，callee 能用 top 推导真实参数个数；
        //   也避免 caller 之前留下的临时值被误当成参数的一部分。
        let func_val = self
            .stack
            .get(func_index)
            .cloned()
            .ok_or_else(|| Self::oob(func_index, self.stack.len()))?;

        let (proto_id, upvalues) = Self::resolve_callable(func_val)?;

        let (base, top_cap, varargs) = self.prepare_callee_frame(proto_id, func_index, nargs)?;

        self.frames.push(CallFrame {
            proto_id: Some(proto_id),
            pc: 0,
            func: return_target,
            base,
            top: top_cap,
            varargs,
            upvalues,
            results,
        });
        Ok(())
    }

    /// 尾调用（Lua: TAILCALL）：用 callee 替换当前 Lua 帧（不增长 frames）。
    ///
    /// 重要语义：
    /// - 新的 callee 会继承“当前函数的返回值规格”（ResultsSpec），从而把结果直接返回给上层 caller
    /// - 参数/变参的整理规则与 `call_with_results` 相同
    pub(crate) fn tail_call(&mut self, func_index: usize, nargs: usize) -> Result<(), VmError> {
        let (return_target, results) = {
            let fr = self.lua_frame()?;
            (fr.func, fr.results)
        };

        let func_val = self
            .stack
            .get(func_index)
            .cloned()
            .ok_or_else(|| Self::oob(func_index, self.stack.len()))?;

        let (proto_id, upvalues) = Self::resolve_callable(func_val)?;

        // TailCall 会替换当前帧；在离开当前帧前必须先封闭其 open upvalue。
        self.close_current_frame_upvalues()?;

        let (base, top_cap, varargs) = self.prepare_callee_frame(proto_id, func_index, nargs)?;

        // 用 callee 替换当前帧（frame-replace）：
        // - func/结果规格保留（它属于“当前函数返回到上层 caller 的写回目标/期望”）
        // - proto/base/top/varargs 切换为 callee 的信息
        //
        // 注意：此时 `stack[func_index]`（被调函数对象）与参数区仍在当前栈上，
        // 只是我们改变了寄存器窗口的 base，让 callee 的 R0 从参数起点开始解释。
        let fr = self.lua_frame_mut()?;
        fr.proto_id = Some(proto_id);
        fr.pc = 0;
        fr.func = return_target;
        fr.base = base;
        fr.top = top_cap;
        fr.varargs = varargs;
        fr.upvalues = upvalues;
        fr.results = results;
        Ok(())
    }

    /// 把可调用值解包为 `(proto_id, upvalues)`。
    ///
    /// - `LFn`：无 upvalues
    /// - `Closure`：携带 upvalue 共享单元
    fn resolve_callable(value: Value) -> Result<(usize, Vec<UpvalueRef>), VmError> {
        match value {
            Value::LFn(pid) => Ok((pid, vec![])),
            Value::Closure { proto_id, upvalues } => Ok((proto_id, upvalues)),
            other => Err(VmError::NotCallable(other)),
        }
    }

    /// 按 Lua 5.x 的规则整理参数/varargs，并计算 callee 的寄存器窗口。
    ///
    /// 返回：(base, top_cap, varargs)
    fn prepare_callee_frame(
        &mut self,
        proto_id: usize,
        func_index: usize,
        nargs: usize,
    ) -> Result<(usize, usize, Vec<Value>), VmError> {
        let (max_stack, num_params, is_vararg) = {
            let proto = self
                .protos
                .get(proto_id)
                .ok_or_else(|| Self::proto_oob(proto_id, self.protos.len()))?;
            (proto.max_stack, proto.num_params, proto.is_vararg)
        };

        let base = func_index + 1;
        let arg_end = base + nargs;

        // Lua 5.x 语义：进入 callee 前，先用 top “封口”参数区（top = base + nargs）。
        // 这样 B=0（变参调用）时，caller 传入的动态参数区间是确定的。
        self.set_top(arg_end);

        // 处理固定参数与变参：
        // - 固定参数区：R0..R(num_params-1)
        // - 若 nargs < num_params：缺省参数填 Nil
        // - 若 nargs > num_params：
        //   - is_vararg=true：多余参数保存到 frame.varargs（供 VARARG 指令拷贝）
        //   - is_vararg=false：多余参数丢弃（不可通过寄存器直接访问）
        let mut varargs: Vec<Value> = vec![];
        if is_vararg && nargs > num_params {
            let start = base + num_params;
            for idx in start..arg_end {
                varargs.push(
                    self.stack
                        .get(idx)
                        .cloned()
                        .ok_or_else(|| Self::oob(idx, self.stack.len()))?,
                );
            }
        }

        // 缺省参数补 Nil（避免读到旧值）。
        if nargs < num_params {
            let needed = base + num_params;
            self.ensure_stack_space(needed);
            for idx in arg_end..needed {
                self.stack[idx] = Value::Nil;
            }
        }

        // 进入 callee 后，“有效 top”只需要覆盖固定参数区（多余参数已经被保存/丢弃）。
        // 这能避免多余参数被 Return B=0 / Call B=0 误当作有效寄存器值。
        self.set_top(base + num_params);

        // 寄存器窗口大小由 proto.max_stack 决定（Lua 编译期会计算它）。
        // 为了稳妥，至少要容纳固定参数寄存器。
        let top_cap = base + max_stack.max(num_params);
        self.ensure_stack_space(top_cap);

        // 把本帧未初始化的寄存器槽位清成 Nil（更接近 Lua 的“空寄存器视为 nil”的直觉，也让测试更稳定）。
        for idx in self.top..top_cap {
            self.stack[idx] = Value::Nil;
        }

        Ok((base, top_cap, varargs))
    }
    fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
        if let Some(s) = payload.downcast_ref::<&'static str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "panic".to_string()
        }
    }

    /// 类似 Lua 的 `pcall`：执行调用并返回第一个返回值（没有返回值则 Nil）。
    pub fn pcall(&mut self, func: usize, nargs: usize, nresults: usize) -> Result<Value, VmError> {
        let mut results = self.pcall_results(func, nargs, nresults)?;
        Ok(results.drain(..).next().unwrap_or(Value::Nil))
    }

    /// Rust 风格的“动态返回”：返回一个 `Vec<Value>`，长度由 `nresults` 决定。
    /// - `nresults == 0`：丢弃返回值（返回空 vec）
    /// - `nresults > 0`：返回 `nresults` 个（不足补 Nil，多余丢弃）
    pub fn pcall_results(
        &mut self,
        func: usize,
        nargs: usize,
        nresults: usize,
    ) -> Result<Vec<Value>, VmError> {
        if func >= self.top {
            return Err(Self::oob(func, self.top));
        }

        let cp = self.checkpoint();
        let r = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<Value>, VmError> {
            // 运行时错误用 Result 表达；这里的 catch_unwind 只是“兜底”捕获意外 panic。
            self.call_with_results(func, nargs, ResultsSpec::Fixed(nresults))?;
            self.run_results()
        }));

        match r {
            Ok(Ok(vs)) => Ok(vs),
            Ok(Err(e)) => {
                self.rollback(cp);
                Err(e)
            }
            Err(payload) => {
                self.rollback(cp);
                Err(VmError::Panic(Self::panic_payload_to_string(payload)))
            }
        }
    }

    /// Lua 风格的“多返回”：相当于 `Call` 的 `C=0`，返回所有结果（可能为 0 个）。
    pub fn pcall_multi(&mut self, func: usize, nargs: usize) -> Result<Vec<Value>, VmError> {
        if func >= self.top {
            return Err(Self::oob(func, self.top));
        }

        let cp = self.checkpoint();
        let r = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<Value>, VmError> {
            self.call_with_results(func, nargs, ResultsSpec::Multi)?;
            self.run_results()
        }));

        match r {
            Ok(Ok(vs)) => Ok(vs),
            Ok(Err(e)) => {
                self.rollback(cp);
                Err(e)
            }
            Err(payload) => {
                self.rollback(cp);
                Err(VmError::Panic(Self::panic_payload_to_string(payload)))
            }
        }
    }
}
