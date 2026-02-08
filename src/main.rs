use study_lua::opcode::{ LoadK,Add, Return};
use study_lua::vm::Vm;
use study_lua::{Operand, Value};

fn main() {
    // 常量表：K0=1, K1=2
    let consts = vec![Value::Number(1.0), Value::Number(2.0)];
    // 寄存器
    let mut regs = vec![Value::Nil; reg_count];
    regs[1] = Value::Number(1.0);

    const reg_count: usize = 8;
    // 对照 demo.lua 的“概念字节码”
    let code = vec![
        LoadK(0, 0),
        Add(1,Operand::R(0).encode(),Operand::K(1).encode()),
        Return(1,0,0)
    ];

    let mut vm = Vm::new(reg_count, consts, code, regs);

    let ret = vm.run();
    println!("ret = {:?}", ret); // 期望 Number(3.0)
}
