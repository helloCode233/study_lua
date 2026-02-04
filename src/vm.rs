use crate::{Instruction, OpCode, Operand, Value};
pub struct Vm {
    regs: Vec<Value>,
    consts: Vec<Value>,
    code: Vec<Instruction>,
    pc: usize,
}



impl Vm {
    pub fn new(reg_count: usize, consts: Vec<Value>, code: Vec<Instruction>,regs:Vec<Value>) -> Self {
        Self {
            regs,
            consts,
            code,
            pc: 0,
        }
    }


    fn as_number(v: &Value) -> f64 {
        match v {
            Value::Number(n) => *n,
            _ => panic!("type error: expected number, got {:?}", v),
        }
    }


    fn read_rk(&self, rk: u32) -> &Value {
        match Operand::decode(rk) {
            Operand::R(v) => self.regs.get(v).unwrap(),
            Operand::K(v) => self.consts.get(v).unwrap(),
        }
    }



    pub fn run(&mut self) -> Value {
        loop {
            let i = self.code[self.pc].clone();
            self.pc += 1;

            match i {
                Instruction::LoadK { dst, k } => {
                    self.regs[dst] = self.consts[k].clone();
                }
                Instruction::Move { dst, src } => {
                    self.regs[dst] = self.regs[src].clone();
                }
                Instruction::AddRK { dst, x, y } => {
                    let a = Self::as_number(self.read_rk(x));
                    let b = Self::as_number(self.read_rk(y));
                    self.regs[dst] = Value::Number(a + b);
                }
                Instruction::Return { src } => {
                    return self.regs[src].clone();
                }
            }
        }
    }
}