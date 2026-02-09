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
    Eq,    // ABC: if (RK[B] == RK[C]) != A then pc++
    Lt,    // ABC: if (RK[B] <  RK[C]) != A then pc++
    Jmp,   // pc += sBx
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
    Return,   // return R[A]
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
pub fn return_(a: u32, b: u32, c: u32) -> u32 {
    abc(6, a, b, c)
}

impl Opcode {
    pub fn decode(i: &u32) -> Result<Opcode, crate::VmError> {
        match op(i) {
            0 => Ok(Opcode::LoadK),
            1 => Ok(Opcode::Move),
            2 => Ok(Opcode::Add),
            3 => Ok(Opcode::Eq),
            4 => Ok(Opcode::Lt),
            5 => Ok(Opcode::Jmp),
            6 => Ok(Opcode::Return),
            7 => Ok(Opcode::Call),
            8 => Ok(Opcode::Vararg),
            9 => Ok(Opcode::TailCall),
            other => Err(crate::VmError::UnknownOpcode(other)),
        }
    }

    pub fn encode(&self) -> u32 {
        match self {
            Opcode::LoadK => 0,
            Opcode::Move => 1,
            Opcode::Add => 2,
            Opcode::Eq => 3,
            Opcode::Lt => 4,
            Opcode::Jmp => 5,
            Opcode::Call => 7,
            Opcode::Vararg => 8,
            Opcode::TailCall => 9,
            Opcode::Return => 6,
        }
    }
}
