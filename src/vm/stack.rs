use crate::opcode::BITRK;
use crate::vm::model::Vm;
use crate::{Value, VmError};

impl Vm {
    pub(crate) fn rget(&self, r: u32) -> Result<&Value, VmError> {
        let fr = self.lua_frame()?;
        let idx = fr.base + r as usize;

        // 对寄存器访问，使用本帧的寄存器窗口上界（frame.top）做边界检查。
        // `Vm.top` 主要用于 B=0/C=0 的变参/多返回语义（“有效 top”），不用于寄存器越界判断。
        if idx >= fr.top {
            return Err(Self::oob(idx, fr.top));
        }

        self.stack
            .get(idx)
            .ok_or_else(|| Self::oob(idx, self.stack.len()))
    }

    pub(crate) fn rset(&mut self, r: u32, v: Value) -> Result<(), VmError> {
        let (base, top_cap) = match self.frames.last() {
            Some(fr) if fr.proto_id.is_some() => (fr.base, fr.top),
            _ => return Err(VmError::NoLuaFrame),
        };
        let idx = base + r as usize;
        if idx >= top_cap {
            return Err(Self::oob(idx, top_cap));
        }
        if idx >= self.stack.len() {
            return Err(Self::oob(idx, self.stack.len()));
        }
        self.stack[idx] = v;
        // 写寄存器会推进“有效 top”，用于 B=0/C=0 的语义判断：
        // - B=0 读取 top 来决定参数末尾
        // - Return B=0 读取 top 来决定多返回的末尾
        // - Call C=0 会在写回后更新 top
        self.top = self.top.max(idx + 1);
        Ok(())
    }

    // 栈扩展函数，确保栈不越界
    pub(crate) fn ensure_stack_space(&mut self, top: usize) {
        if self.stack.len() < top {
            self.stack.resize(top, Value::Nil);
        }
    }

    /// 设置“有效 top”（Lua 风格：指向第一个空槽位）。
    ///
    /// 语义说明：
    /// - `Vm.top` 用于 B=0/C=0 的变参/多返回规则（见 `model.rs` 的说明）。
    /// - 这里不会缩小 `stack.len()`（它更像容量/可访问区间），只会更新 `Vm.top`。
    ///
    /// 细节（为了更贴近 Lua 5.x，并避免“读取过期值”导致测试不稳定）：
    /// - 当 new_top 变小：把 `[new_top, old_top)` 之间的槽位清成 `Nil`，表示这些槽位不再有效。
    /// - 当 new_top 变大：确保栈有足够空间，并保持新增槽位为 `Nil`。
    pub(crate) fn set_top(&mut self, new_top: usize) {
        if new_top > self.stack.len() {
            self.ensure_stack_space(new_top);
        }

        if new_top < self.top {
            let end = self.top.min(self.stack.len());
            for idx in new_top..end {
                self.stack[idx] = Value::Nil;
            }
        }

        self.top = new_top;
    }

    // RK：用 BITRK 来判断
    pub(crate) fn rk_get(&self, proto_id: usize, x: u32) -> Result<Value, VmError> {
        if (x & BITRK) != 0 {
            let k = (x & !BITRK) as usize;
            let proto = self
                .protos
                .get(proto_id)
                .ok_or_else(|| Self::proto_oob(proto_id, self.protos.len()))?;
            proto
                .consts
                .get(k)
                .cloned()
                .ok_or_else(|| Self::oob(k, proto.consts.len()))
        } else {
            Ok(self.rget(x)?.clone())
        }
    }
}
