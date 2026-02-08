use crate::opcode::{Opcode, a, b, bx, c, sbx};
use crate::{ Operand, Value};

pub struct Vm {
    regs: Vec<Value>,
    consts: Vec<Value>,
    code: Vec<u32>,
    pc: isize,
}

impl Vm {
    pub fn new(reg_count: usize, consts: Vec<Value>, code: Vec<u32>, regs: Vec<Value>) -> Self {
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
            let i = &self.code[self.pc as usize];
            self.pc += 1;

            match Opcode::decode(i) {
                Opcode::LoadK => {
                    self.regs[a(i) as usize] = self.consts[bx(i) as usize].clone();
                }
                Opcode::Move => {
                    self.regs[a(i) as usize] = self.regs[b(i) as usize].clone();
                }
                // 当 a = false  eq == true  jmp ==
                // 当 a = true   eq == false jmp !=
                Opcode::Eq => {
                    let x = Self::as_number(self.read_rk(b(i)));
                    let y = Self::as_number(self.read_rk(c(i)));
                    if (x == y) != (a(i) == 1) {
                        self.pc += 1;
                    }
                }
                Opcode::Add => {
                    let x = Self::as_number(self.read_rk(b(i)));
                    let y = Self::as_number(self.read_rk(c(i)));
                    self.regs[a(i) as usize] = Value::Number(x + y);
                }
                Opcode::Return => {
                    return self.regs[a(i) as usize].clone();
                }
                Opcode::Jmp => {
                    self.pc += sbx(i) as isize;
                }
                // true <  false >
                Opcode::Lt => {
                    let x = Self::as_number(self.read_rk(b(i)));
                    let y = Self::as_number(self.read_rk(c(i)));
                    let lt = x < y;
                    if lt != (a(i) == 1) {
                        self.pc += 1;
                    }
                }
            }
        }
    }
}
