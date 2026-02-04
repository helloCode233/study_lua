use study_lua::{Instruction, OpCode, Operand, Value};
use study_lua::vm::{Vm};

fn main() {
    // 常量表：K0=1, K1=2
    let consts = vec![Value::Number(1.0), Value::Number(2.0)];
    // 寄存器
    let mut regs = vec![Value::Nil; reg_count];
    regs[1]=Value::Number(1.0);

    const  reg_count: usize = 8;
    // 对照 demo.lua 的“概念字节码”
    let code = vec![
        Instruction::LoadK { dst: 0, k: 0 },        // R0 = K0 (a=1)
        Instruction::AddRK {
            dst: 1,
            x: Operand::R(0).encode(), // a 在寄存器
            y: Operand::K(1).encode(), // 1 在常量表
        },    // R2 = R0 + R1 (c=a+b)
        Instruction::Return { src: 1 },             // return R2
    ];

    let mut vm = Vm::new(reg_count,consts,code,regs);

    let ret = vm.run();
    println!("ret = {:?}", ret); // 期望 Number(3.0)
}
