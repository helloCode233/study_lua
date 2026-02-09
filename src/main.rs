use study_lua::opcode::{add, load_k, return_};
use study_lua::proto::Proto;
use study_lua::vm::Vm;
use study_lua::{Value, rk_k, rk_r};

fn main() {
    // 常量表：K0=1, K1=2
    let consts = vec![Value::Number(1.0), Value::Number(2.0)];

    // 对照 demo.lua 的“概念字节码”
    let code = vec![load_k(0, 0), add(1, rk_r(0), rk_k(1)), return_(1, 0, 0)];
    let main_proto = Proto {
        code,
        consts,
        num_params: 0,
        is_vararg: false,
        max_stack: 2,
    };
    let mut vm = Vm::new(vec![main_proto]);

    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    println!("ret = {:?}", ret); // 期望 Number(3.0)
}
