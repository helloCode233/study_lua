use crate::vm::model::{UpvalueCell, UpvalueRef, Vm};
use crate::{Value, VmError};
use std::cell::RefCell;
use std::rc::Rc;

impl Vm {
    /// 捕获一个栈槽位为 upvalue（Open）。
    ///
    /// 如果该槽位已被其它闭包捕获，则复用同一个 upvalue 单元，
    /// 从而实现“多个闭包共享同一外层局部变量”。
    pub(crate) fn capture_upvalue(&mut self, stack_index: usize) -> UpvalueRef {
        if let Some(existing) = self.open_upvalues.get(&stack_index) {
            return Rc::clone(existing);
        }

        let upvalue = Rc::new(RefCell::new(UpvalueCell::Open { stack_index }));
        self.open_upvalues.insert(stack_index, Rc::clone(&upvalue));
        upvalue
    }

    /// 读取 upvalue 当前值（Open 读栈，Closed 读封箱值）。
    pub(crate) fn read_upvalue(&self, upvalue: &UpvalueRef) -> Result<Value, VmError> {
        match upvalue.borrow().clone() {
            UpvalueCell::Open { stack_index } => self
                .stack
                .get(stack_index)
                .cloned()
                .ok_or_else(|| Self::oob(stack_index, self.stack.len())),
            UpvalueCell::Closed(v) => Ok(v),
        }
    }

    /// 写 upvalue（Open 写回栈，Closed 更新封箱值）。
    pub(crate) fn write_upvalue(
        &mut self,
        upvalue: &UpvalueRef,
        value: Value,
    ) -> Result<(), VmError> {
        let open_index = match &*upvalue.borrow() {
            UpvalueCell::Open { stack_index } => Some(*stack_index),
            UpvalueCell::Closed(_) => None,
        };

        if let Some(stack_index) = open_index {
            if stack_index >= self.stack.len() {
                return Err(Self::oob(stack_index, self.stack.len()));
            }
            self.stack[stack_index] = value;
            self.top = self.top.max(stack_index + 1);
            return Ok(());
        }

        *upvalue.borrow_mut() = UpvalueCell::Closed(value);
        Ok(())
    }

    /// 封闭单个 open upvalue：把它从栈引用变成独立值。
    pub(crate) fn close_upvalue(&mut self, stack_index: usize) -> Result<(), VmError> {
        let Some(upvalue) = self.open_upvalues.remove(&stack_index) else {
            return Ok(());
        };

        let value = self
            .stack
            .get(stack_index)
            .cloned()
            .ok_or_else(|| Self::oob(stack_index, self.stack.len()))?;
        *upvalue.borrow_mut() = UpvalueCell::Closed(value);
        Ok(())
    }

    /// 封闭 `stack_index >= start` 的所有 open upvalue。
    ///
    /// 这个边界规则对应 Lua `CLOSE A` 和函数返回时的“离开作用域”行为。
    pub(crate) fn close_upvalues_from(&mut self, start: usize) -> Result<(), VmError> {
        let keys: Vec<usize> = self.open_upvalues.range(start..).map(|(k, _)| *k).collect();
        for stack_index in keys {
            self.close_upvalue(stack_index)?;
        }
        Ok(())
    }

    /// 封闭当前帧内的全部 open upvalue（frame.base 及以上）。
    pub(crate) fn close_current_frame_upvalues(&mut self) -> Result<(), VmError> {
        let frame_base = self.lua_frame()?.base;
        self.close_upvalues_from(frame_base)
    }
}
