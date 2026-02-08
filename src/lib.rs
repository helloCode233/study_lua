pub mod opcode;
pub mod vm;

#[derive(Clone, Debug)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Nil,
}


#[derive(Clone, Debug)]
pub enum Operand {
    R(usize), // register index
    K(usize), // const index
}
// 0 2 4 8 16
// (1+2+4+8)+1 = 16
impl Operand {
    pub fn encode(&self) -> u32 {
        match self {
            Operand::R(v) => *v as u32,
            Operand::K(v) => 0x8000_0000 | (*v as u32),
        }
    }
    pub fn decode(v: u32) -> Operand {
        // if (v & 0x7fff_ffff)>>31 == 1 {
        if (v & 0x8000_0000) != 0 {
            Operand::K((v & 0x7fff_ffff) as usize)
        } else {
            Operand::R(v as usize)
        }
    }
}
