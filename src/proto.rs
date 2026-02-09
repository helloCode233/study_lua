use crate::Value;

pub struct Proto {
    pub code: Vec<u32>,
    pub consts: Vec<Value>,
    /// 该函数固定参数个数（不包含 `...`）。
    ///
    /// Lua 5.x 中，固定参数会被分配到寄存器（R0..R(num_params-1)）。
    pub num_params: usize,
    /// 是否为变参函数（是否声明了 `...`）。
    ///
    /// - `is_vararg == false`：多余参数会被丢弃（不可通过寄存器直接访问）
    /// - `is_vararg == true`：多余参数会被保存为 varargs，并通过 `VARARG` 指令拷贝到寄存器
    pub is_vararg: bool,
    /// 该函数需要的寄存器数量上界（寄存器窗口大小）。
    ///
    /// 注意：这里用 `usize` 简化实现，后面你可以 assert <= 256。
    pub max_stack: usize,
}
