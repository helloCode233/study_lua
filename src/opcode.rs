pub const SIZE_OP: u32 = 6;
pub const SIZE_A: u32 = 8;
pub const SIZE_B: u32 = 9;
pub const SIZE_C: u32 = 9;
pub const SIZE_BX: u32 = SIZE_B + SIZE_C;

pub const POS_OP: u32 = 0;
pub const POS_A: u32 = POS_OP + SIZE_OP;
pub const POS_C: u32 = POS_A + SIZE_A;
pub const POS_B: u32 = POS_C + SIZE_C;
pub const POS_BX: u32 = POS_C;

pub const BITRK: u32 = 1 << (SIZE_B - 1); // 256

// 计算Bx字段的最大无符号值
pub const MARG_BX: u32 = (1u32 << SIZE_BX) - 1;
// 计算sBx字段的最大正数值
pub const MARG_SBX: i32 = (MARG_BX as i32) >> 1;

#[inline]
fn mask(size: u32) -> u32 {
    (1u32 << size) - 1
}

#[inline]
fn get(i: &u32, pos: u32, size: u32) -> u32 {
    (i >> pos) & mask(size)
}

#[inline]
fn set(i: u32, pos: u32, size: u32, v: u32) -> u32 {
    let m = mask(size) << pos;
    (i & !m) | ((v & mask(size)) << pos)
}

#[inline]
pub fn op(i: &u32) -> u32 {
    get(i, POS_OP, SIZE_OP)
}
#[inline]
pub fn a(i: &u32) -> u32 {
    get(i, POS_A, SIZE_A)
}
#[inline]
pub fn b(i: &u32) -> u32 {
    get(i, POS_B, SIZE_B)
}
#[inline]
pub fn c(i: &u32) -> u32 {
    get(i, POS_C, SIZE_C)
}
#[inline]
pub fn bx(i: &u32) -> u32 {
    get(i, POS_BX, SIZE_BX)
}
#[inline]
pub fn sbx(i: &u32) -> i32 {
    bx(i) as i32 - MARG_SBX
}

#[inline]
pub fn abx(op: u32, a: u32, bx: u32) -> u32 {
    let mut i = 0;
    i = set(i, POS_OP, SIZE_OP, op);
    i = set(i, POS_A, SIZE_A, a);
    i = set(i, POS_BX, SIZE_BX, bx);
    i
}
#[inline]
pub fn abc(op: u32, a: u32, b: u32, c: u32) -> u32 {
    let mut i = 0;
    i = set(i, POS_OP, SIZE_OP, op);
    i = set(i, POS_A, SIZE_A, a);
    i = set(i, POS_B, SIZE_B, b);
    i = set(i, POS_C, SIZE_C, c);
    i
}
#[inline]
pub fn asbx(op: u32, a: u32, sbx: i32) -> u32 {
    let mut i = 0;
    i = set(i, POS_OP, SIZE_OP, op);
    i = set(i, POS_A, SIZE_A, a);
    i = set(i, POS_BX, SIZE_BX, (sbx + MARG_SBX) as u32);
    i
}

pub enum Opcode {
    LoadK, // R[A] = K[B]
    Move,  // R[A] = R[B]
    Add,   // R[A] = R[B] + R[C]
    Mul,   // R[A] = R[B] * R[C]
    Div,   // R[A] = R[B] / R[C]
    Eq,    // ABC: if (RK[B] == RK[C]) != A then pc++
    Lt,    // ABC: if (RK[B] <  RK[C]) != A then pc++
    // Lua 风格：Test A C
    //
    // - 用寄存器真值参与条件分支：
    //   if (truthy(R[A]) != C) then pc++
    //
    // 常与后续 Jmp 搭配，形成 if/else 的“条件跳过下一条跳转”模板。
    Test, // ABC
    Jmp,  // pc += sBx
    // Lua 风格：Call A B C
    // - 函数槽位：R[A]
    // - 参数个数：B-1（当 B=0 时，参数来自 R[A+1]..R[top-1]）
    // - 返回个数：C-1（当 C=0 时，多返回，写回后 caller.top 会被更新）
    Call, // ABC: R[A](R[A+1]...R[A+B-1])  B=nargs+1  C=nresults+1
    // Lua 风格：Vararg A B
    // - 从当前帧的 varargs 列表拷贝到寄存器
    // - B != 0：拷贝 B-1 个
    // - B == 0：拷贝全部
    Vararg, // ABC: R[A].. = vararg
    // Lua 风格：TailCall A B C
    //
    // 语义类似 Call，但属于“尾调用优化”：不会新建调用帧，而是用 callee 替换当前帧，
    // 并把“当前函数的返回值规格”（ResultsSpec）原样继承给 callee。
    //
    // 这让 `return f(...)` 这类尾递归/尾调用在 VM 内部不增长 frames。
    TailCall, // ABC: return R[A](...)  (frame-replace)
    // Lua 风格：TForCall A C
    //
    // 泛型 for 的“迭代器调用”阶段：
    // - 调用 R[A](R[A+1], R[A+2])（即 generator(state, control)）
    // - 把返回值写到 R[A+3]..R[A+2+C]
    //
    // 注意：写回起点不是函数槽位，因此执行层需要用“自定义返回写回目标”。
    TForCall, // ABC: R[A+3],...,R[A+2+C] := R[A](R[A+1], R[A+2])
    // Lua 风格：TForLoop A sBx
    //
    // 迭代判定阶段：
    // - 若 R[A+3] != nil，则 R[A+2] = R[A+3] 并跳转（继续循环体）
    // - 否则不跳转（结束循环）
    TForLoop, // AsBx
    // Lua 风格：Closure A Bx
    //
    // - 以原型表中的 proto(Bx) 构造闭包
    // - upvalue 捕获规则来自 child proto 的 `upvalues` 描述
    Closure, // ABx: R[A] = closure(proto[Bx])
    // Lua 风格：GetUpval A B
    //
    // - R[A] = upvalue[B]
    GetUpval, // ABC
    // Lua 风格：SetUpval A B
    //
    // - upvalue[B] = R[A]
    SetUpval, // ABC
    // Lua 风格：Close A
    //
    // - 关闭当前栈帧中 `R[A]` 及其之上的 open upvalue
    Close,  // ABC
    Return, // return R[A]
}

// 指令编码辅助函数（按 Rust 习惯使用 snake_case 命名）。
#[inline]
pub fn load_k(a: u32, bx: u32) -> u32 {
    abx(0, a, bx)
}
#[inline]
pub fn move_(a: u32, b: u32, c: u32) -> u32 {
    abc(1, a, b, c)
}
#[inline]
pub fn add(a: u32, b: u32, c: u32) -> u32 {
    abc(2, a, b, c)
}
#[inline]
pub fn mul(a: u32, b: u32, c: u32) -> u32 {
    abc(16, a, b, c)
}
#[inline]
pub fn div(a: u32, b: u32, c: u32) -> u32 {
    abc(17, a, b, c)
}
#[inline]
pub fn eq(a: u32, b: u32, c: u32) -> u32 {
    abc(3, a, b, c)
}
#[inline]
pub fn lt(a: u32, b: u32, c: u32) -> u32 {
    abc(4, a, b, c)
}
#[inline]
pub fn jmp(a: u32, bx: u32) -> u32 {
    abx(5, a, bx)
}
#[inline]
pub fn jmp_sbx(a: u32, sbx: i32) -> u32 {
    asbx(5, a, sbx)
}
#[inline]
pub fn test_(a: u32, c: u32) -> u32 {
    abc(18, a, 0, c)
}
#[inline]
pub fn call(a: u32, b: u32, c: u32) -> u32 {
    abc(7, a, b, c)
}
#[inline]
pub fn vararg(a: u32, b: u32, c: u32) -> u32 {
    abc(8, a, b, c)
}
#[inline]
pub fn tail_call(a: u32, b: u32, c: u32) -> u32 {
    abc(9, a, b, c)
}
#[inline]
pub fn tfor_call(a: u32, c: u32) -> u32 {
    abc(10, a, 0, c)
}
#[inline]
pub fn tfor_loop(a: u32, sbx: i32) -> u32 {
    asbx(11, a, sbx)
}
#[inline]
pub fn closure(a: u32, bx: u32) -> u32 {
    abx(12, a, bx)
}
#[inline]
pub fn get_upval(a: u32, b: u32) -> u32 {
    abc(13, a, b, 0)
}
#[inline]
pub fn set_upval(a: u32, b: u32) -> u32 {
    abc(14, a, b, 0)
}
#[inline]
pub fn close(a: u32) -> u32 {
    abc(15, a, 0, 0)
}
#[inline]
pub fn return_(a: u32, b: u32, c: u32) -> u32 {
    abc(6, a, b, c)
}

impl Opcode {
    pub fn decode(i: &u32) -> Result<Opcode, crate::VmError> {
        match op(i) {
            0 => Ok(Opcode::LoadK),
            1 => Ok(Opcode::Move),
            2 => Ok(Opcode::Add),
            16 => Ok(Opcode::Mul),
            17 => Ok(Opcode::Div),
            3 => Ok(Opcode::Eq),
            4 => Ok(Opcode::Lt),
            18 => Ok(Opcode::Test),
            5 => Ok(Opcode::Jmp),
            6 => Ok(Opcode::Return),
            7 => Ok(Opcode::Call),
            8 => Ok(Opcode::Vararg),
            9 => Ok(Opcode::TailCall),
            10 => Ok(Opcode::TForCall),
            11 => Ok(Opcode::TForLoop),
            12 => Ok(Opcode::Closure),
            13 => Ok(Opcode::GetUpval),
            14 => Ok(Opcode::SetUpval),
            15 => Ok(Opcode::Close),
            other => Err(crate::VmError::UnknownOpcode(other)),
        }
    }

    pub fn encode(&self) -> u32 {
        match self {
            Opcode::LoadK => 0,
            Opcode::Move => 1,
            Opcode::Add => 2,
            Opcode::Mul => 16,
            Opcode::Div => 17,
            Opcode::Eq => 3,
            Opcode::Lt => 4,
            Opcode::Test => 18,
            Opcode::Jmp => 5,
            Opcode::Call => 7,
            Opcode::Vararg => 8,
            Opcode::TailCall => 9,
            Opcode::TForCall => 10,
            Opcode::TForLoop => 11,
            Opcode::Closure => 12,
            Opcode::GetUpval => 13,
            Opcode::SetUpval => 14,
            Opcode::Close => 15,
            Opcode::Return => 6,
        }
    }
}
