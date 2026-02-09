use crate::VmError;
use crate::vm::model::{CallFrame, Vm, VmCheckpoint};

impl Vm {
    pub(crate) fn fr(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }
    pub(crate) fn fr_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    pub(crate) fn lua_frame(&self) -> Result<&CallFrame, VmError> {
        let fr = self.fr();
        if fr.proto_id.is_some() {
            Ok(fr)
        } else {
            Err(VmError::NoLuaFrame)
        }
    }

    pub(crate) fn lua_frame_mut(&mut self) -> Result<&mut CallFrame, VmError> {
        let is_lua = self.frames.last().is_some_and(|f| f.proto_id.is_some());
        if is_lua {
            Ok(self.fr_mut())
        } else {
            Err(VmError::NoLuaFrame)
        }
    }

    pub(crate) fn checkpoint(&self) -> VmCheckpoint {
        VmCheckpoint {
            top: self.top,
            frames_len: self.frames.len(),
        }
    }

    pub(crate) fn rollback(&mut self, cp: VmCheckpoint) {
        // 回滚“有效 top”（Lua 风格：指向第一个空槽位）。
        // 注意：这里不能用 `truncate(top)` 来同步 Vec.len，因为寄存器窗口需要更大的容量/可访问区间：
        // - `Vm.top` 只表示“有效 top”（用于 B=0/C=0 的变参/多返回语义）
        // - `stack.len()` 更像容量；收缩 len 可能导致后续寄存器访问越界
        self.set_top(cp.top);
        self.frames.truncate(cp.frames_len.max(1));
    }
}
