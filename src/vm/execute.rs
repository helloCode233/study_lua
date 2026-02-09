use crate::opcode::{Opcode, a, b, bx, c, sbx};
use crate::vm::model::ResultsSpec;
use crate::vm::model::Vm;
use crate::{Value, VmError};

impl Vm {
    /// 运行当前 Lua 帧直到返回，并以“动态返回”语义返回所有结果：
    /// - 如果本次调用的结果规格是 Fixed(n)：返回 n 个（不足补 Nil，多余丢弃）
    /// - 如果是 Multi：返回所有结果（可能为 0 个）
    pub fn run_results(&mut self) -> Result<Vec<Value>, VmError> {
        loop {
            let (proto_id, pc) = {
                let fr = self.lua_frame()?;
                let proto_id = fr.proto_id.unwrap();
                let pc = usize::try_from(fr.pc).unwrap_or(usize::MAX);
                (proto_id, pc)
            };

            let i = {
                let proto = self
                    .protos
                    .get(proto_id)
                    .ok_or_else(|| Self::proto_oob(proto_id, self.protos.len()))?;
                let code_len = proto.code.len();
                if pc >= code_len {
                    return Err(VmError::PcOutOfBounds { pc, code_len });
                }
                proto.code[pc]
            };

            match Opcode::decode(&i)? {
                Opcode::LoadK => {
                    let k = bx(&i) as usize;
                    let v = {
                        let proto = self
                            .protos
                            .get(proto_id)
                            .ok_or_else(|| Self::proto_oob(proto_id, self.protos.len()))?;
                        proto
                            .consts
                            .get(k)
                            .cloned()
                            .ok_or_else(|| Self::oob(k, proto.consts.len()))?
                    };
                    self.rset(a(&i), v)?;
                }
                Opcode::Move => {
                    let v = self.rget(b(&i))?.clone();
                    self.rset(a(&i), v)?;
                }
                // 当 a = false  eq == true  jmp ==
                // 当 a = true   eq == false jmp !=
                Opcode::Eq => {
                    let x = Self::as_number(&self.rk_get(proto_id, b(&i))?)?;
                    let y = Self::as_number(&self.rk_get(proto_id, c(&i))?)?;
                    if (x == y) != (a(&i) == 1) {
                        self.lua_frame_mut()?.pc += 1;
                    }
                }
                Opcode::Add => {
                    let x = Self::as_number(&self.rk_get(proto_id, b(&i))?)?;
                    let y = Self::as_number(&self.rk_get(proto_id, c(&i))?)?;
                    self.rset(a(&i), Value::Number(x + y))?;
                }
                Opcode::Mul => {
                    let x = Self::as_number(&self.rk_get(proto_id, b(&i))?)?;
                    let y = Self::as_number(&self.rk_get(proto_id, c(&i))?)?;
                    self.rset(a(&i), Value::Number(x * y))?;
                }
                Opcode::Div => {
                    let x = Self::as_number(&self.rk_get(proto_id, b(&i))?)?;
                    let y = Self::as_number(&self.rk_get(proto_id, c(&i))?)?;
                    self.rset(a(&i), Value::Number(x / y))?;
                }
                Opcode::Vararg => {
                    let a_field = a(&i);
                    let b_field = b(&i);

                    // Vararg A B（Lua 风格）：
                    // - B != 0：拷贝 B-1 个 vararg 到 R[A]..R[A+B-2]
                    // - B == 0：拷贝全部 vararg 到 R[A]..（数量由 varargs.len() 决定）
                    //
                    // 注意：vararg 值来自 call 时保存的 `CallFrame.varargs`（非寄存器窗口直接可见）。
                    let varargs = { self.lua_frame()?.varargs.clone() };
                    let n = if b_field == 0 {
                        varargs.len()
                    } else {
                        b_field as usize - 1
                    };

                    for i in 0..n {
                        self.rset(
                            a_field + i as u32,
                            varargs.get(i).cloned().unwrap_or(Value::Nil),
                        )?;
                    }
                }
                Opcode::Closure => {
                    let a_field = a(&i);
                    let child_proto_id = bx(&i) as usize;

                    // CLOSURE A Bx（Lua 风格，简化实现）：
                    // - 根据 child proto 的 upvalue 描述创建闭包
                    // - instack=true：捕获当前寄存器槽位（open upvalue，可共享）
                    // - instack=false：从当前函数自身 upvalues 转发同一个共享单元
                    let up_descs = {
                        let proto = self
                            .protos
                            .get(child_proto_id)
                            .ok_or_else(|| Self::proto_oob(child_proto_id, self.protos.len()))?;
                        proto.upvalues.clone()
                    };
                    let (frame_base, frame_top, parent_upvalues) = {
                        let fr = self.lua_frame()?;
                        (fr.base, fr.top, fr.upvalues.clone())
                    };
                    let mut captured = Vec::with_capacity(up_descs.len());
                    for desc in up_descs {
                        if desc.instack {
                            let stack_index = frame_base + desc.index;
                            if stack_index >= frame_top {
                                return Err(Self::oob(stack_index, frame_top));
                            }
                            captured.push(self.capture_upvalue(stack_index));
                        } else {
                            captured.push(parent_upvalues.get(desc.index).cloned().ok_or_else(
                                || VmError::UpvalueOutOfBounds {
                                    index: desc.index,
                                    len: parent_upvalues.len(),
                                },
                            )?);
                        }
                    }
                    self.rset(
                        a_field,
                        Value::Closure {
                            proto_id: child_proto_id,
                            upvalues: captured,
                        },
                    )?;
                }
                Opcode::GetUpval => {
                    let a_field = a(&i);
                    let b_field = b(&i) as usize;

                    // GETUPVAL A B：R[A] = upvalue[B]
                    let (upvalue, len) = {
                        let fr = self.lua_frame()?;
                        (fr.upvalues.get(b_field).cloned(), fr.upvalues.len())
                    };
                    let upvalue = upvalue.ok_or(VmError::UpvalueOutOfBounds {
                        index: b_field,
                        len,
                    })?;
                    let value = self.read_upvalue(&upvalue)?;
                    self.rset(a_field, value)?;
                }
                Opcode::SetUpval => {
                    let a_field = a(&i);
                    let b_field = b(&i) as usize;

                    // SETUPVAL A B：upvalue[B] = R[A]
                    let value = self.rget(a_field)?.clone();
                    let (upvalue, len) = {
                        let fr = self.lua_frame()?;
                        (fr.upvalues.get(b_field).cloned(), fr.upvalues.len())
                    };
                    let upvalue = upvalue.ok_or(VmError::UpvalueOutOfBounds {
                        index: b_field,
                        len,
                    })?;
                    self.write_upvalue(&upvalue, value)?;
                }
                Opcode::Close => {
                    let a_field = a(&i) as usize;

                    // CLOSE A：关闭当前帧中 `R[A]` 及其以上槽位关联的 open upvalue。
                    let close_from = {
                        let fr = self.lua_frame()?;
                        fr.base + a_field
                    };
                    self.close_upvalues_from(close_from)?;
                }
                Opcode::Call => {
                    let b_field = b(&i);
                    let c_field = c(&i);
                    // Call A B C（Lua 风格）：
                    // - 函数槽位在 R[A]
                    // - 参数槽位从 R[A+1] 开始
                    // - B 决定参数个数：
                    //   - B != 0：参数个数 = B-1（固定参数）
                    //   - B == 0：参数来自 R[A+1]..R[top-1]（由“有效 top”决定）
                    // - C 决定返回值规格：
                    //   - C != 0：写回 C-1 个（不足补 Nil，多余丢弃）
                    //   - C == 0：多返回（写回后会更新 top）
                    let results = if c_field == 0 {
                        ResultsSpec::Multi
                    } else {
                        ResultsSpec::Fixed(c_field as usize - 1)
                    };
                    let func_index = {
                        let fr = self.lua_frame()?;
                        fr.base + a(&i) as usize
                    };
                    let nargs = if b_field == 0 {
                        let args_start = func_index + 1;
                        // 注意：`self.top` 是“有效 top”，可能大于寄存器窗口上界（frame.top），
                        // 这是为了模拟 Lua 在 B=0/C=0 场景下用 top 来描述“动态参数/多返回”的行为。
                        if self.top >= args_start {
                            self.top - args_start
                        } else {
                            0
                        }
                    } else {
                        b_field as usize - 1
                    };

                    // 先把 caller 的 pc 移到下一条，再切换到 callee（避免统一的 pc++ 影响 callee）。
                    self.lua_frame_mut()?.pc += 1;
                    self.call_with_results(func_index, nargs, results)?;
                    continue;
                }
                Opcode::TailCall => {
                    let b_field = b(&i);

                    // TailCall A B C（Lua 风格，简化实现）：
                    // - 参数规则与 Call 相同（B=0 使用 top 决定参数个数）
                    // - 返回值规格不取决于 C：tailcall 会继承“当前函数的返回值规格”，
                    //   让 callee 的 Return 直接返回给上层 caller（frame-replace）。
                    //
                    // 为什么要这么做：
                    // - Lua 的 `return f(...)` 是典型尾调用场景
                    // - 如果每次都 push 新帧，尾递归会线性增长 frames（容易爆栈/性能差）
                    // - TAILCALL 通过“替换当前帧”实现尾调用优化，让 frames 不增长
                    let func_index = {
                        let fr = self.lua_frame()?;
                        fr.base + a(&i) as usize
                    };
                    let nargs = if b_field == 0 {
                        let args_start = func_index + 1;
                        if self.top >= args_start {
                            self.top - args_start
                        } else {
                            0
                        }
                    } else {
                        b_field as usize - 1
                    };

                    self.tail_call(func_index, nargs)?;
                    continue;
                }
                Opcode::TForCall => {
                    let a_field = a(&i) as usize;
                    let c_field = c(&i) as usize;
                    let (base, func_index) = {
                        let fr = self.lua_frame()?;
                        let base = fr.base;
                        (base, base + a_field)
                    };

                    // TFORCALL A C（Lua 5.x 泛型 for）：
                    // - 调用 generator：R[A](R[A+1], R[A+2])
                    // - 把结果写回 R[A+3]..（共 C 个）
                    //
                    // 这里用 `call_with_results_target` 指定“返回值写回起点”，
                    // 避免普通 CALL 把结果写回函数槽位。
                    let return_target = base + a_field + 3;
                    self.lua_frame_mut()?.pc += 1;
                    self.call_with_results_target(
                        func_index,
                        2,
                        ResultsSpec::Fixed(c_field),
                        return_target,
                    )?;
                    continue;
                }
                Opcode::TForLoop => {
                    let a_field = a(&i) as usize;
                    let (base, jump_delta) = {
                        let fr = self.lua_frame()?;
                        (fr.base, sbx(&i) as isize)
                    };
                    let idx_pos = base + a_field + 3;
                    let ctrl_pos = base + a_field + 2;
                    let idx_val = self
                        .stack
                        .get(idx_pos)
                        .cloned()
                        .ok_or_else(|| Self::oob(idx_pos, self.stack.len()))?;

                    // TFORLOOP A sBx（Lua 5.x）：
                    // - 若 R(A+3) != nil：把它回写到 R(A+2) 并跳转到循环体
                    // - 若 R(A+3) == nil：不跳转，循环结束
                    if !matches!(idx_val, Value::Nil) {
                        self.ensure_stack_space(ctrl_pos + 1);
                        self.stack[ctrl_pos] = idx_val;
                        self.lua_frame_mut()?.pc += jump_delta;
                    }
                }
                Opcode::Return => {
                    let (func_index, results, base) = {
                        let fr = self.lua_frame()?;
                        (fr.func, fr.results, fr.base)
                    };
                    let a_field = a(&i);
                    let b_field = b(&i);

                    // Return A B C（本项目只用到 A/B）：
                    // - B != 0：固定返回 B-1 个值，从 R[A] 起连续取
                    // - B == 0：多返回：返回 R[A]..R[top-1]（top 使用“有效 top”）
                    // 先把本帧的返回值拷出来，再 pop 帧并写回到 caller 的栈窗口。
                    let rets: Vec<Value> = if b_field == 0 {
                        // 多返回：R(A)..R(top-1)，这里的 top 使用 Vm.top（有效 top）。
                        let start = base + a_field as usize;
                        let end = self.top;
                        if end < start {
                            vec![]
                        } else {
                            let mut out = Vec::with_capacity(end - start);
                            for idx in start..end {
                                out.push(
                                    self.stack
                                        .get(idx)
                                        .cloned()
                                        .ok_or_else(|| Self::oob(idx, self.stack.len()))?,
                                );
                            }
                            out
                        }
                    } else {
                        // 固定返回：B-1 个
                        let nret = b_field as usize - 1;
                        let mut out = Vec::with_capacity(nret);
                        for rr in a_field..(a_field + nret as u32) {
                            out.push(self.rget(rr)?.clone());
                        }
                        out
                    };

                    // 函数返回会离开当前栈帧；必须先封闭本帧 open upvalue，
                    // 避免后续 top 收缩后 upvalue 仍悬挂到无效栈槽位。
                    self.close_current_frame_upvalues()?;

                    // 弹出 callee。
                    self.frames.pop();

                    if func_index >= self.stack.len() {
                        return Err(Self::oob(func_index, self.stack.len()));
                    }

                    // 把返回值写回 caller 的 `func_index` 起始位置，并更新 caller 的“有效 top”。
                    //
                    // Lua 5.x 行为要点（对应 CALL 的 C 字段）：
                    // - C != 0（Fixed）：返回后 top = ra + (C-1)
                    // - C == 0（Multi）：返回后 top = ra + nret
                    // - C == 1（Fixed(0)）：丢弃返回值，top = ra
                    match results {
                        ResultsSpec::Fixed(0) => {
                            // 丢弃返回值（但仍把函数槽位覆盖为 Nil，避免残留）。
                            self.stack[func_index] = Value::Nil;
                            self.set_top(func_index);
                        }
                        ResultsSpec::Fixed(n) => {
                            // 固定写回 n 个：不足补 Nil，多余丢弃。
                            let needed = func_index + n;
                            self.ensure_stack_space(needed);
                            for i in 0..n {
                                self.stack[func_index + i] =
                                    rets.get(i).cloned().unwrap_or(Value::Nil);
                            }
                            self.set_top(func_index + n);
                        }
                        ResultsSpec::Multi => {
                            // 多返回：写回全部返回值，并把 `top` 更新到写回末尾。
                            // 如果返回 0 个值，则把 `top` 退回到 `func_index`（Lua 风格）。
                            if rets.is_empty() {
                                self.stack[func_index] = Value::Nil;
                                self.set_top(func_index);
                            } else {
                                let needed = func_index + rets.len();
                                self.ensure_stack_space(needed);
                                for (i, v) in rets.iter().cloned().enumerate() {
                                    self.stack[func_index + i] = v;
                                }
                                self.set_top(needed);
                            }
                        }
                    }

                    // 如果已经回到最外层，Return 就是整个 VM 的返回值。
                    if self.frames.len() == 1 {
                        // 这里直接根据“调用时指定的 results 规格”来返回 Vec（与 Lua C API 类似）：
                        // - Fixed(n)：返回 n 个（不足补 Nil）
                        // - Multi：返回所有
                        return Ok(match results {
                            ResultsSpec::Fixed(0) => vec![],
                            ResultsSpec::Fixed(n) => {
                                let mut out = Vec::with_capacity(n);
                                for i in 0..n {
                                    out.push(rets.get(i).cloned().unwrap_or(Value::Nil));
                                }
                                out
                            }
                            ResultsSpec::Multi => rets,
                        });
                    }

                    continue;
                }
                Opcode::Jmp => {
                    self.lua_frame_mut()?.pc += sbx(&i) as isize;
                }
                // true <  false >
                Opcode::Lt => {
                    let x = Self::as_number(&self.rk_get(proto_id, b(&i))?)?;
                    let y = Self::as_number(&self.rk_get(proto_id, c(&i))?)?;
                    let lt = x < y;
                    if lt != (a(&i) == 1) {
                        self.lua_frame_mut()?.pc += 1;
                    }
                }
                Opcode::Test => {
                    let truthy = !matches!(self.rget(a(&i))?, Value::Nil | Value::Bool(false));
                    let expected = c(&i) == 1;
                    if truthy != expected {
                        self.lua_frame_mut()?.pc += 1;
                    }
                }
            }
            self.lua_frame_mut()?.pc += 1;
        }
    }

    /// 运行直到 Return，并只取第一个返回值（没有返回值则为 Nil）。
    pub fn run(&mut self) -> Result<Value, VmError> {
        let mut results = self.run_results()?;
        Ok(results.drain(..).next().unwrap_or(Value::Nil))
    }
}
