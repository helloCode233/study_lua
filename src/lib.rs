pub mod vm;

#[derive(Clone, Debug)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Nil,
}

#[derive(Clone, Copy, Debug)]
pub enum OpCode {
    LoadK,   // R[A] = K[B]
    Move,    // R[A] = R[B]
    Add,     // R[A] = R[B] + R[C]
    Jmp,     // pc += sBx
    Return,  // return R[A]
}

#[derive(Clone, Debug)]
pub enum Instruction {
    LoadK { dst: usize, k: usize },        // R[dst] = K[k]
    Move  { dst: usize, src: usize },      // R[dst] = R[src]
    AddRK { dst: usize, x: u32, y: u32 },// R[dst] = R[x] + R[y]
    Return { src: usize },                 // return R[src]
}



#[derive(Clone, Debug)]
pub enum Operand {
    R(usize), // register index
    K(usize), // const index
}
// 0 2 4 8 16
// (1+2+4+8)+1 = 16
impl Operand {
    pub fn encode(& self) -> u32 {
        match self {
            Operand::R(v) => { *v as u32 }
            Operand::K(v) => 0x8000_0000 |(*v as u32),
        }
    }
    pub fn decode (v: u32) -> Operand {
        // if (v & 0x7fff_ffff)>>31 == 1 {
        if (v & 0x8000_0000) != 0 {
            Operand::K((v & 0x7fff_ffff) as usize)
        }else {
            Operand::R(v as usize)
        }
    }
}