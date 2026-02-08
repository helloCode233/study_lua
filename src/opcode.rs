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
    LoadK,  // R[A] = K[B]
    Move,   // R[A] = R[B]
    Add,    // R[A] = R[B] + R[C]
    Eq,     // ABC: if (RK[B] == RK[C]) != A then pc++
    Lt,     // ABC: if (RK[B] <  RK[C]) != A then pc++
    Jmp,    // pc += sBx
    Return, // return R[A]
}

#[inline]
pub fn LoadK(a: u32, bx: u32) -> u32 {
    abx(0, a, bx)
}
#[inline]
pub fn Move(a: u32, b: u32, c: u32) -> u32 {
    abc(1, a, b, c)
}
#[inline]
pub fn Add(a: u32, b: u32, c: u32) -> u32 {
    abc(2, a, b, c)
}
#[inline]
pub fn Eq(a: u32, b: u32, c: u32) -> u32 {
    abc(3, a, b, c)
}
#[inline]
pub fn Lt(a: u32, b: u32, c: u32) -> u32 {
    abc(4, a, b, c)
}
#[inline]
pub fn Jmp(a: u32, bx: u32) -> u32 {
    abx(5, a, bx)
}
#[inline]
pub fn Return(a: u32, b: u32, c: u32) -> u32 {
    abc(6, a, b, c)
}

impl Opcode {
    pub fn decode(i: &u32) -> Opcode {
        match op(i) {
            0 => Opcode::LoadK,
            1 => Opcode::Move,
            2 => Opcode::Add,
            3 => Opcode::Eq,
            4 => Opcode::Lt,
            5 => Opcode::Jmp,
            6 => Opcode::Return,
            _ => {
                panic!("Opcode::from(): unknown opcode: {}", i);
            }
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
            Opcode::Return => 6,
        }
    }
}
