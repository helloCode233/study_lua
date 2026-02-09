use study_lua::opcode::{
    add, call, close, closure, get_upval, load_k, return_, set_upval, tail_call, tfor_call,
    tfor_loop, vararg,
};
use study_lua::proto::{Proto, UpvalueDesc};

use study_lua::vm::Vm;
use study_lua::{Value, VmError, rk_k, rk_r};

fn push(vm: &mut Vm, v: Value) -> usize {
    let idx = vm.top;
    if vm.stack.len() == idx {
        vm.stack.push(v);
    } else {
        vm.stack[idx] = v;
    }
    vm.top += 1;
    idx
}

fn make_add_proto(k0: Value, k1: Value) -> Proto {
    let consts = vec![k0, k1];
    let code = vec![load_k(0, 0), add(1, rk_r(0), rk_k(1)), return_(1, 0, 0)];
    Proto {
        code,
        consts,
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    }
}

#[test]
fn pcall_calls_stack_function() {
    let main_proto = make_add_proto(Value::Number(1.0), Value::Number(2.0));
    let mut vm = Vm::new(vec![main_proto]);
    let func = vm.load(0).unwrap();

    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 3.0).abs() < 1e-9));
}

#[test]
fn pcall_results_returns_multiple_values() {
    // f(): return 1, 2
    let f_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    let mut vm = Vm::new(vec![f_proto]);
    let func = vm.load(0).unwrap();

    let results = vm.pcall_results(func, 0, 2).unwrap();
    assert_eq!(results.len(), 2);
    assert!(matches!(results[0], Value::Number(n) if (n - 1.0).abs() < 1e-9));
    assert!(matches!(results[1], Value::Number(n) if (n - 2.0).abs() < 1e-9));
}

#[test]
fn pcall_multi_returns_all_values_or_empty() {
    // f0(): return 1, 2
    let f0_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };
    // f1(): return nothing (B=1 => 0 results)
    let f1_proto = Proto {
        code: vec![return_(0, 1, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 0,
    };

    let mut vm = Vm::new(vec![f0_proto, f1_proto]);

    let func0 = vm.load(0).unwrap();
    let results0 = vm.pcall_multi(func0, 0).unwrap();
    assert_eq!(results0.len(), 2);

    let func1 = vm.load(1).unwrap();
    let results1 = vm.pcall_multi(func1, 0).unwrap();
    assert!(results1.is_empty());
}

#[test]
fn pcall_type_error_returns_err_and_no_panic() {
    let main_proto = make_add_proto(Value::Bool(true), Value::Number(2.0));
    let mut vm = Vm::new(vec![main_proto]);
    let func = vm.load(0).unwrap();

    let err = vm.pcall(func, 0, 1).unwrap_err();
    match err {
        VmError::TypeError { expected, got } => {
            assert_eq!(expected, "number");
            assert!(matches!(got, Value::Bool(true)));
        }
        other => panic!("expected TypeError, got {other:?}"),
    }
}

#[test]
fn pcall_unknown_opcode_returns_err() {
    let main_proto = Proto {
        code: vec![63],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };
    let mut vm = Vm::new(vec![main_proto]);
    let func = vm.load(0).unwrap();

    let err = vm.pcall(func, 0, 1).unwrap_err();
    match err {
        VmError::UnknownOpcode(63) => {}
        other => panic!("expected UnknownOpcode(63), got {other:?}"),
    }
}

#[test]
fn closure_captures_parent_local_with_getupval() {
    // Lua 对照：
    //
    //   local function outer()
    //     local x = 41
    //     return function()
    //       return x
    //     end
    //   end
    //
    //   local f = outer()
    //   return f()
    //
    // 这里验证：
    // - CLOSURE 会按 child proto 的 upvalue 描述捕获父寄存器
    // - GETUPVAL 能从闭包环境读到捕获值

    // inner(): return upvalue[0]
    let inner_proto = Proto {
        code: vec![get_upval(0, 0), return_(0, 2, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 0,
        }],
        max_stack: 1,
    };

    // outer(): x=41; return closure(inner)
    let outer_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = 41
            closure(1, 2), // R1 = closure(inner)
            return_(1, 2, 0),
        ],
        consts: vec![Value::Number(41.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main(): f=outer(); return f()
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = outer
            call(0, 1, 2), // R0 = outer()
            call(0, 1, 2), // R0 = f()
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };

    // proto 顺序：0=main, 1=outer, 2=inner
    let mut vm = Vm::new(vec![main_proto, outer_proto, inner_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 41.0).abs() < 1e-9));
}

#[test]
fn closure_can_forward_parent_upvalue_to_grandchild() {
    // Lua 对照：
    //
    //   local function outer()
    //     local x = 42
    //     return function()
    //       return function()
    //         return x
    //       end
    //     end
    //   end
    //
    //   return outer()()()
    //
    // 这里验证：
    // - middle 闭包先从 outer 捕获 x（instack=true）
    // - inner 闭包再从 middle 的 upvalues 转发捕获（instack=false）

    // inner(): return upvalue[0]
    let inner_proto = Proto {
        code: vec![get_upval(0, 0), return_(0, 2, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: false,
            index: 0,
        }],
        max_stack: 1,
    };

    // middle(): return closure(inner)
    let middle_proto = Proto {
        code: vec![
            closure(0, 3), // R0 = closure(inner), 捕获 middle.upvalues[0]
            return_(0, 2, 0),
        ],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 0,
        }],
        max_stack: 1,
    };

    // outer(): x=42; return closure(middle)
    let outer_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = 42
            closure(1, 2), // R1 = closure(middle), 捕获 R0
            return_(1, 2, 0),
        ],
        consts: vec![Value::Number(42.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main(): return outer()()()
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = outer
            call(0, 1, 2), // R0 = outer()
            call(0, 1, 2), // R0 = middle()
            call(0, 1, 2), // R0 = inner()
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };

    // proto 顺序：0=main, 1=outer, 2=middle, 3=inner
    let mut vm = Vm::new(vec![main_proto, outer_proto, middle_proto, inner_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 42.0).abs() < 1e-9));
}

#[test]
fn vararg_function_can_read_extra_args_via_vararg_opcode() {
    // f(...): local a,b = ...; return a + b
    //
    // - num_params=0, is_vararg=true：所有传入参数都进入 varargs 列表
    // - VARARG 0 3：拷贝 2 个 vararg 到 R0/R1
    let f_proto = Proto {
        code: vec![
            vararg(0, 3, 0),          // R0,R1 = ...
            add(0, rk_r(0), rk_r(1)), // R0 = R0 + R1
            return_(0, 2, 0),         // return R0
        ],
        consts: vec![],
        num_params: 0,
        is_vararg: true,
        upvalues: vec![],
        max_stack: 2,
    };

    // main(): return f(10, 20) using B=0 (动态参数区间由 top 决定)
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = f
            load_k(1, 1),  // R1 = 10
            load_k(2, 2),  // R2 = 20
            call(0, 0, 2), // R0 = f(R1..R(top-1))
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(10.0), Value::Number(20.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 30.0).abs() < 1e-9));
}

#[test]
fn non_vararg_function_discards_extra_args() {
    // h(a, b): return a + b + c
    //
    // 这里的 c 是一个“局部寄存器”(R2)，不是参数。
    // 如果 VM 没有丢弃多余参数，那么 caller 传入的第 3 个参数可能会污染 R2，
    // 导致 h(1,2,100) 不报错（错误地返回 103）。
    let h_proto = Proto {
        code: vec![
            add(0, rk_r(0), rk_r(1)), // R0 = a + b
            add(0, rk_r(0), rk_r(2)), // R0 = (a + b) + c  (c 应为 Nil)
            return_(0, 2, 0),
        ],
        consts: vec![],
        num_params: 2,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    // main(): call h(1,2,100) using B=0
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = h
            load_k(1, 1),  // R1 = 1
            load_k(2, 2),  // R2 = 2
            load_k(3, 3),  // R3 = 100（多余参数，应当被丢弃）
            call(0, 0, 2), // R0 = h(R1..R(top-1))
            return_(0, 2, 0),
        ],
        consts: vec![
            Value::LFn(1),
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(100.0),
        ],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 4,
    };

    let mut vm = Vm::new(vec![main_proto, h_proto]);
    let func = vm.load(0).unwrap();
    let err = vm.pcall(func, 0, 1).unwrap_err();
    match err {
        VmError::TypeError { expected, got } => {
            assert_eq!(expected, "number");
            assert!(matches!(got, Value::Nil));
        }
        other => panic!("expected TypeError(number, Nil), got {other:?}"),
    }
}

#[test]
fn tailcall_returns_values_to_caller_without_extra_return_frame() {
    // f(): return 1, 2
    let f_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // g(): return f()   (通过 TAILCALL 实现)
    //
    // 说明：
    // - TAILCALL 会用 f() 替换当前帧，因此 g 自己不需要再执行 Return。
    // - 这里额外放一个 `return` 作为兜底：若 tailcall 实现有 bug 导致继续执行到下一条，
    //   测试会失败（返回空而不是期望的值）。
    let g_proto = Proto {
        code: vec![
            load_k(0, 0),       // R0 = f
            tail_call(0, 1, 0), // tailcall f()
            return_(0, 1, 0),   // should be unreachable
        ],
        consts: vec![Value::LFn(2)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };

    // main(): x,y = g(); return x + y
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = g
            call(0, 1, 0), // R0.. = g()  (C=0 多返回)
            add(2, rk_r(0), rk_r(1)),
            return_(2, 2, 0),
        ],
        consts: vec![Value::LFn(1)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    // protos: [main, g, f]
    let mut vm = Vm::new(vec![main_proto, g_proto, f_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 3.0).abs() < 1e-9));
}

#[test]
fn lua_script_compare_vararg_tailcall_call_chain() {
    // Lua 脚本（对照）：
    //
    //   local function add(a, b)
    //     return a + b
    //   end
    //
    //   local function forward(...)
    //     return add(...)
    //   end
    //
    //   return forward(10, 20)
    //
    // 这个例子同时覆盖三件事：
    // 1) `...` 通过 VARARG 从 frame.varargs 拷贝到寄存器
    // 2) `return add(...)` 使用 TAILCALL（不新增调用帧）
    // 3) 外层调用使用 B=0，让参数个数由 top 动态决定

    // add(a, b): return a + b
    let add_proto = Proto {
        code: vec![add(0, rk_r(0), rk_r(1)), return_(0, 2, 0)],
        consts: vec![],
        num_params: 2,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // forward(...): return add(...)
    //
    // 字节码思路：
    // - R0 = add
    // - R1.. = ...      (VARARG B=0 => 拷贝全部)
    // - tailcall R0(...) (TAILCALL B=0 => 参数区由 top 决定)
    let forward_proto = Proto {
        code: vec![
            load_k(0, 0),       // R0 = add
            vararg(1, 0, 0),    // R1.. = ...
            tail_call(0, 0, 0), // return add(...)
        ],
        consts: vec![Value::LFn(2)],
        num_params: 0,
        is_vararg: true,
        upvalues: vec![],
        max_stack: 4,
    };

    // main(): return forward(10, 20)
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = forward
            load_k(1, 1),  // R1 = 10
            load_k(2, 2),  // R2 = 20
            call(0, 0, 2), // R0 = forward(R1..R(top-1))
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(10.0), Value::Number(20.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    // proto 顺序：
    // 0 = main, 1 = forward, 2 = add
    let mut vm = Vm::new(vec![main_proto, forward_proto, add_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 30.0).abs() < 1e-9));
}

#[test]
fn same_script_c_field_changes_return_propagation() {
    // Lua 脚本（概念对照，只差“接收方式”）：
    //
    //   local function pair()
    //     return 1, 2
    //   end
    //
    //   -- 版本 A（固定接收）：local a = pair(); return a + 100
    //   -- 版本 B（多返回接收）：local a, b = pair(); return a + b
    //
    // 在字节码层面，关键差异就是 Call 的 C 字段：
    // - C != 0（这里 C=2）：固定接收 1 个返回值
    // - C == 0：多返回传播（写回数量由被调函数 Return 决定）

    // pair(): return 1, 2
    let pair_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main_fixed:
    //   a = pair()      -- Call C=2 => 只接收 1 个返回值
    //   return a + 100
    let main_fixed_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = pair
            call(0, 1, 2), // R0 = pair()   (C=2 => 1 result)
            load_k(1, 1),  // R1 = 100
            add(0, rk_r(0), rk_r(1)),
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(100.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main_multi:
    //   a, b = pair()   -- Call C=0 => 多返回传播
    //   return a + b
    let main_multi_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = pair
            call(0, 1, 0), // R0.. = pair() (C=0 => multi results)
            add(2, rk_r(0), rk_r(1)),
            return_(2, 2, 0),
        ],
        consts: vec![Value::LFn(1)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    // 场景 A：固定接收（C=2）=> 1 + 100 = 101
    let mut vm_fixed = Vm::new(vec![main_fixed_proto, pair_proto]);
    let func_fixed = vm_fixed.load(0).unwrap();
    let ret_fixed = vm_fixed.pcall(func_fixed, 0, 1).unwrap();
    assert!(matches!(ret_fixed, Value::Number(n) if (n - 101.0).abs() < 1e-9));

    // 场景 B：多返回传播（C=0）=> 1 + 2 = 3
    let pair_proto_2 = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };
    let mut vm_multi = Vm::new(vec![main_multi_proto, pair_proto_2]);
    let func_multi = vm_multi.load(0).unwrap();
    let ret_multi = vm_multi.pcall(func_multi, 0, 1).unwrap();
    assert!(matches!(ret_multi, Value::Number(n) if (n - 3.0).abs() < 1e-9));
}

#[test]
fn tforcall_writes_to_a_plus_3_and_truncates_by_c() {
    // Lua 对照（泛型 for 的“调用迭代器”阶段）：
    //
    //   -- 迭代器返回 3 个值：next_ctrl, value, extra
    //   local function gen(state, ctrl)
    //     return ctrl + 1, 80, 999
    //   end
    //
    //   -- 对应 TFORCALL A C，C=2：
    //   -- 只接收前 2 个返回值到 R(A+3), R(A+4)
    //
    // 这个测试验证：
    // 1) TFORCALL 的写回起点是 A+3（不是函数槽位 A）
    // 2) C 控制固定接收个数（多余返回值会被丢弃）

    let gen_proto = Proto {
        code: vec![
            add(0, rk_r(1), rk_k(0)), // R0 = ctrl + 1
            load_k(1, 1),             // R1 = 80
            load_k(2, 2),             // R2 = 999
            return_(0, 4, 0),         // return R0,R1,R2
        ],
        consts: vec![
            Value::Number(1.0),
            Value::Number(80.0),
            Value::Number(999.0),
        ],
        num_params: 2,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    // main 布局（A=0）：
    // - R0 = generator
    // - R1 = state
    // - R2 = control
    // - TFORCALL 结果写到 R3,R4
    let main_proto = Proto {
        code: vec![
            load_k(0, 0), // R0 = gen
            load_k(1, 1), // R1 = state(0)
            load_k(2, 2), // R2 = control(7)
            tfor_call(0, 2),
            add(5, rk_r(3), rk_r(4)), // R5 = R3 + R4 => 8 + 80 = 88
            return_(5, 2, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(0.0), Value::Number(7.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 6,
    };

    let mut vm = Vm::new(vec![main_proto, gen_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 88.0).abs() < 1e-9));
}

#[test]
fn tforloop_copies_index_and_branches_on_non_nil() {
    // Lua 对照（泛型 for 的“循环判定”阶段）：
    //
    //   -- 这里直接构造：R(A+3) 由 TFORCALL 产生
    //   -- TFORLOOP: if R(A+3) ~= nil then R(A+2)=R(A+3); pc += sBx end
    //
    // 用两个子场景验证：
    // 1) next_idx 非 nil：发生跳转，并把 control 更新为 next_idx
    // 2) next_idx 为 nil：不跳转（走 false 分支）

    // 子场景 1：gen_non_nil(state, ctrl) => ctrl+1, 80
    let gen_non_nil = Proto {
        code: vec![
            add(0, rk_r(1), rk_k(0)), // next_idx = ctrl + 1
            load_k(1, 1),             // value = 80
            return_(0, 3, 0),         // return next_idx, value
        ],
        consts: vec![Value::Number(1.0), Value::Number(80.0)],
        num_params: 2,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    let main_non_nil = Proto {
        code: vec![
            load_k(0, 0), // R0 = gen_non_nil
            load_k(1, 1), // R1 = state(0)
            load_k(2, 2), // R2 = control(7)
            tfor_call(0, 2),
            tfor_loop(0, 2), // true 时跳到 pc=7
            load_k(0, 3),    // false 分支标记（不应执行）
            return_(0, 2, 0),
            return_(2, 2, 0), // true 分支：返回更新后的 control（应为 next_idx=8）
        ],
        consts: vec![
            Value::LFn(1),
            Value::Number(0.0),
            Value::Number(7.0),
            Value::Number(-1.0),
        ],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 5,
    };

    let mut vm_non_nil = Vm::new(vec![main_non_nil, gen_non_nil]);
    let func_non_nil = vm_non_nil.load(0).unwrap();
    let ret_non_nil = vm_non_nil.pcall(func_non_nil, 0, 1).unwrap();
    assert!(matches!(ret_non_nil, Value::Number(n) if (n - 8.0).abs() < 1e-9));

    // 子场景 2：gen_nil(state, ctrl) => nil, 80
    let gen_nil = Proto {
        code: vec![
            load_k(0, 0),     // next_idx = nil
            load_k(1, 1),     // value = 80
            return_(0, 3, 0), // return next_idx, value
        ],
        consts: vec![Value::Nil, Value::Number(80.0)],
        num_params: 2,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    let main_nil = Proto {
        code: vec![
            load_k(0, 0), // R0 = gen_nil
            load_k(1, 1), // R1 = state(0)
            load_k(2, 2), // R2 = control(7)
            tfor_call(0, 2),
            tfor_loop(0, 2), // false：不跳（应走到下一条）
            load_k(0, 3),    // false 分支标记
            return_(0, 2, 0),
            return_(2, 2, 0), // true 分支（不应执行）
        ],
        consts: vec![
            Value::LFn(1),
            Value::Number(0.0),
            Value::Number(7.0),
            Value::Number(42.0),
        ],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 5,
    };

    let mut vm_nil = Vm::new(vec![main_nil, gen_nil]);
    let func_nil = vm_nil.load(0).unwrap();
    let ret_nil = vm_nil.pcall(func_nil, 0, 1).unwrap();
    assert!(matches!(ret_nil, Value::Number(n) if (n - 42.0).abs() < 1e-9));
}

#[test]
fn nested_call_via_call_opcode() {
    // f(): return 1
    let f_proto = Proto {
        code: vec![load_k(0, 0), return_(0, 0, 0)],
        consts: vec![Value::Number(1.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };

    // main(): return f() + 2
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = f
            call(0, 1, 2), // R0 = f()
            load_k(1, 1),  // R1 = 2
            add(0, rk_r(0), rk_r(1)),
            return_(0, 0, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto]);
    let func = vm.load(0).unwrap();

    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 3.0).abs() < 1e-9));
}

#[test]
fn call_b0_uses_top_for_args() {
    // f(a, b): return a + b
    let f_proto = Proto {
        code: vec![add(0, rk_r(0), rk_r(1)), return_(0, 2, 0)],
        consts: vec![],
        num_params: 2,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main(): return f(10, 20) using B=0
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = f
            load_k(1, 1),  // R1 = 10
            load_k(2, 2),  // R2 = 20   (写到 R2 会把 Vm.top 推到包含两个参数)
            call(0, 0, 2), // R0 = f(R1..R(top-1))
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(10.0), Value::Number(20.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto]);
    let func = vm.load(0).unwrap();

    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 30.0).abs() < 1e-9));
}

#[test]
fn call_fixed_args_clear_missing_params_to_nil() {
    // g(a, b, c): return a + b + c
    //
    // 这个用例验证 Lua 5.x 的一个重要细节：
    // - 当以“固定参数个数”调用（B!=0）时，callee 未收到的参数寄存器应当被视为 Nil
    // - caller 在 CALL 前会把 top 设为参数末尾，从而“清掉”多余的临时槽位（避免把旧值当成参数）
    //
    // 如果 VM 没有按 Lua 的 top 规则收缩/清理，那么第三个参数位置可能残留旧值，
    // 导致 g(a,b,c) 在只传 2 个参数时仍然“看见”一个伪造的 c，从而错误地不报错。
    let g_proto = Proto {
        code: vec![
            add(0, rk_r(0), rk_r(1)), // R0 = a + b
            add(0, rk_r(0), rk_r(2)), // R0 = (a + b) + c
            return_(0, 2, 0),
        ],
        consts: vec![],
        num_params: 3,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    // main():
    //   g(1, 2)  (只传 2 个参数)
    //   但在 CALL 前先把 R3 填成 100，模拟“旧值/临时值”占用在参数区后面
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = g
            load_k(1, 1),  // R1 = 1
            load_k(2, 2),  // R2 = 2
            load_k(3, 3),  // R3 = 100（如果 CALL 不收缩 top，这个可能污染 callee 的缺省参数）
            call(0, 3, 2), // 调用 g：B=3 => 2 args（R1,R2），C=2 => 1 result
            return_(0, 2, 0),
        ],
        consts: vec![
            Value::LFn(1),
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(100.0),
        ],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 4,
    };

    let mut vm = Vm::new(vec![main_proto, g_proto]);
    let func = vm.load(0).unwrap();

    // 正确行为：c 缺省为 Nil，因此在 add 中触发 TypeError。
    let err = vm.pcall(func, 0, 1).unwrap_err();
    match err {
        VmError::TypeError { expected, got } => {
            assert_eq!(expected, "number");
            assert!(matches!(got, Value::Nil));
        }
        other => panic!("expected TypeError(number, Nil), got {other:?}"),
    }
}

#[test]
fn call_fixed_results_updates_top_for_return_b0() {
    // f(): return 1, 2
    let f_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main():
    //   R0 = f
    //   R2 = 99      (模拟“旧值/临时值”把 top 推高)
    //   R0 = f()     (C=2 => 固定 1 个返回值)
    //   return ...   (Return B=0：返回 R0..R(top-1)，因此非常依赖 top 是否被正确更新)
    //
    // Lua 5.x 规则：CALL 的 C!=0 时，返回后 top = ra + (C-1)。
    // 所以这里应该只返回 1 个值；如果 top 没被收缩，可能会把 R1/R2 的旧值也返回出去。
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = f
            load_k(2, 1),  // R2 = 99  (推进 top)
            call(0, 1, 2), // R0 = f()  (固定 1 个返回值)
            return_(0, 0, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(99.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto]);
    let func = vm.load(0).unwrap();

    let rets = vm.pcall_multi(func, 0).unwrap();
    assert_eq!(rets.len(), 1);
    assert!(matches!(rets[0], Value::Number(n) if (n - 1.0).abs() < 1e-9));
}

#[test]
fn call_fixed_results_updates_top_for_next_call_b0() {
    // f(): return 1, 2
    let f_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // g(a, b, c): return a + b + c
    // 若只收到 2 个参数，则 c 应为 Nil，从而在 add 中触发 TypeError。
    let g_proto = Proto {
        code: vec![
            add(0, rk_r(0), rk_r(1)),
            add(0, rk_r(0), rk_r(2)),
            return_(0, 2, 0),
        ],
        consts: vec![],
        num_params: 3,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    // main():
    //   R0 = g
    //   R1 = f
    //   R3 = 100      (模拟旧值，把 top 推高)
    //   f() -> R1,R2  (C=3 => 固定 2 个返回值；Lua 应把 top 更新到 R3，不包含旧的 R3 槽位)
    //   g(R1..R(top-1))  (B=0：参数区完全依赖 top)
    //
    // 若 top 没有按 Lua 规则更新/清理，g 会“多拿到”一个第三参数（100）而不报错。
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = g
            load_k(1, 1),  // R1 = f
            load_k(3, 2),  // R3 = 100（旧值）
            call(1, 1, 3), // R1,R2 = f()
            call(0, 0, 2), // R0 = g(R1..R(top-1))
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(2), Value::LFn(1), Value::Number(100.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 4,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto, g_proto]);
    let func = vm.load(0).unwrap();

    let err = vm.pcall(func, 0, 1).unwrap_err();
    match err {
        VmError::TypeError { expected, got } => {
            assert_eq!(expected, "number");
            assert!(matches!(got, Value::Nil));
        }
        other => panic!("expected TypeError(number, Nil), got {other:?}"),
    }
}

#[test]
fn instruction_table_compare_fixed_return_then_b0_call() {
    // Lua 脚本（对照）：
    //
    //   local function pair()
    //     return 1, 2
    //   end
    //
    //   local function sum2(a, b)
    //     return a + b
    //   end
    //
    //   local function main()
    //     local f = pair
    //     local g = sum2
    //     local junk = 99
    //     local x, y = f()   -- 固定接收 2 个返回值（C=3）
    //     return g(x, y)     -- B=0，参数个数由 top 决定
    //   end
    //
    // 执行轨迹表（main 寄存器视角；R0..R3，top 是 Vm.top）：
    // 步骤 | 指令                | 关键状态变化
    // 1    | load_k R0,sum2      | R0=g
    // 2    | load_k R1,pair      | R1=f
    // 3    | load_k R3,99        | R3=99，top 被推高
    // 4    | call R1 B=1 C=3     | f() -> R1,R2；固定 2 返回后 top 应更新到 R3 的前一个空位
    // 5    | call R0 B=0 C=2     | 用 R1..R(top-1) 作为参数，只应看到 R1,R2，不应把旧 R3=99 当参数
    // 6    | return R0           | 结果应为 3
    //
    // 绝对栈索引映射图（帮助理解“寄存器窗口 + 统一值栈”）：
    //
    // 设最外层 `pcall(func=0, nargs=0)`，则：
    // - stack[0]   : main 函数对象（调用槽位 func）
    // - main.base  : 1
    // - main.R0    <=> stack[1]
    // - main.R1    <=> stack[2]
    // - main.R2    <=> stack[3]
    // - main.R3    <=> stack[4]
    //
    // 第 4 步 `call(1,1,3)`（调用 R1=pair）时：
    // - 被调函数槽位 func_index = main.base + 1 = 2   (stack[2] 是 pair)
    // - pair.base = func_index + 1 = 3                 (pair.R0 <=> stack[3])
    // - pair 返回 2 个值后写回 stack[2], stack[3]      (即 main.R1, main.R2)
    //
    // 第 5 步 `call(0,0,2)`（调用 R0=sum2, B=0）时：
    // - 实参数量取决于 top：args = R1..R(top-1)
    // - 由于上一步 fixed(2) 已把 top 收敛到 R3 前，实参只会是 (R1, R2)
    // - 这正是该测试想验证的重点：旧的 R3=99 不应泄漏进 B=0 调用参数。

    // pair(): return 1, 2
    let pair_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // sum2(a, b): return a + b
    let sum2_proto = Proto {
        code: vec![add(0, rk_r(0), rk_r(1)), return_(0, 2, 0)],
        consts: vec![],
        num_params: 2,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main(): 按上面的轨迹执行
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = sum2
            load_k(1, 1),  // R1 = pair
            load_k(3, 2),  // R3 = 99（旧值）
            call(1, 1, 3), // R1,R2 = pair()
            call(0, 0, 2), // R0 = sum2(R1..R(top-1))
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(2), Value::LFn(1), Value::Number(99.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 4,
    };

    // proto 顺序：0=main, 1=pair, 2=sum2
    let mut vm = Vm::new(vec![main_proto, pair_proto, sum2_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();

    // 断言 1：脚本返回值正确（证明 B=0 的参数传播没有被旧 R3 污染）。
    assert!(matches!(ret, Value::Number(n) if (n - 3.0).abs() < 1e-9));
    // 断言 2：最外层 fixed(1) 返回后，top 回到函数槽位之后（便于观察 top 规则是否稳定）。
    assert_eq!(vm.top, 1);
}

#[test]
fn call_c0_multi_returns_and_updates_top() {
    // f(): return 1, 2  (Return B=3 => 2 results)
    let f_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main(): x, y = f(); return x + y  (Call C=0 => Multi)
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = f
            call(0, 1, 0), // R0.. = f()
            add(2, rk_r(0), rk_r(1)),
            return_(2, 2, 0),
        ],
        consts: vec![Value::LFn(1)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto]);
    let func = vm.load(0).unwrap();

    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 3.0).abs() < 1e-9));
}

#[test]
fn call_c0_multi_results_can_feed_next_call_b0() {
    // f(): return 1, 2
    let f_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // g(a, b): return a + b
    let g_proto = Proto {
        code: vec![add(0, rk_r(0), rk_r(1)), return_(0, 2, 0)],
        consts: vec![],
        num_params: 2,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main(): return g(f())
    //
    // Lua 风格求值顺序（寄存器示意）：
    // - R0 = g
    // - R1 = f
    // - Call R1, B=1, C=0  => f() 多返回写回到 R1.. 并更新 top
    // - Call R0, B=0, C=2  => g(R1..R(top-1))
    // - return R0
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = g
            load_k(1, 1),  // R1 = f
            call(1, 1, 0), // f()
            call(0, 0, 2), // g(f())
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(2), Value::LFn(1)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto, g_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 3.0).abs() < 1e-9));
}

#[test]
fn call_fixed_results_fill_nil_when_callee_returns_less() {
    // f(): return 1
    let f_proto = Proto {
        code: vec![load_k(0, 0), return_(0, 2, 0)],
        consts: vec![Value::Number(1.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };

    // main():
    //   a, b = f()      (C=3 => 2 results)
    //   return a + b    (b 应该是 Nil => TypeError)
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = f
            call(0, 1, 3), // R0, R1 = f()
            add(2, rk_r(0), rk_r(1)),
            return_(2, 2, 0),
        ],
        consts: vec![Value::LFn(1)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto]);
    let func = vm.load(0).unwrap();
    let err = vm.pcall(func, 0, 1).unwrap_err();
    match err {
        VmError::TypeError { expected, got } => {
            assert_eq!(expected, "number");
            assert!(matches!(got, Value::Nil));
        }
        other => panic!("expected TypeError(number, Nil), got {other:?}"),
    }
}

#[test]
fn call_fixed_results_drop_extra_when_callee_returns_more() {
    // f(): return 1, 2
    let f_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    // main(): return f() + 2  (Call C=2 => 1 result，只取第一个返回值)
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = f
            call(0, 1, 2), // R0 = f()
            load_k(1, 1),  // R1 = 2
            add(0, rk_r(0), rk_r(1)),
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 3.0).abs() < 1e-9));
}

#[test]
fn call_discard_results_c1_does_not_break_following_code() {
    // f(): return 99
    let f_proto = Proto {
        code: vec![load_k(0, 0), return_(0, 2, 0)],
        consts: vec![Value::Number(99.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };

    // main():
    //   f()            (C=1 => 0 results / discard)
    //   return 7
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = f
            call(0, 1, 1), // discard
            load_k(0, 1),  // R0 = 7
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(7.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 7.0).abs() < 1e-9));
}

#[test]
fn pcall_not_callable_returns_err() {
    let main_proto = make_add_proto(Value::Number(1.0), Value::Number(2.0));
    let mut vm = Vm::new(vec![main_proto]);
    let func = push(&mut vm, Value::Number(1.0));

    let err = vm.pcall(func, 0, 1).unwrap_err();
    match err {
        VmError::NotCallable(Value::Number(_)) => {}
        other => panic!("expected NotCallable(Number), got {other:?}"),
    }
}

#[test]
fn pcall_rolls_back_stack_and_frames_on_error() {
    // f(): return true  (will cause type error in main when adding)
    let f_proto = Proto {
        code: vec![load_k(0, 0), return_(0, 0, 0)],
        consts: vec![Value::Bool(true)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };

    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = f
            call(0, 1, 2), // R0 = f()
            load_k(1, 1),  // R1 = 2
            add(0, rk_r(0), rk_r(1)),
            return_(0, 0, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };

    let mut vm = Vm::new(vec![main_proto, f_proto]);
    push(&mut vm, Value::Number(42.0));
    let func = vm.load(0).unwrap();

    let top_before = vm.top;
    let frames_len_before = vm.frames.len();

    let _ = vm.pcall(func, 0, 1);

    assert_eq!(vm.top, top_before);
    assert_eq!(vm.frames.len(), frames_len_before);
}

#[test]
fn closure_shared_cell_between_two_closures() {
    // Lua 对照：
    //
    //   local function outer()
    //     local x = 1
    //     local function get() return x end
    //     local function set(v) x = v end
    //     set(5)
    //     return get()
    //   end
    //   return outer()
    //
    // 这里验证“open upvalue 共享单元”：
    // - get/set 同时捕获同一个局部变量 x
    // - outer 尚未返回时，set 对 x 的写入可被 get 立即读到
    let get_proto = Proto {
        code: vec![get_upval(0, 0), return_(0, 2, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 0,
        }],
        max_stack: 1,
    };
    let set_proto = Proto {
        code: vec![set_upval(0, 0), return_(0, 1, 0)],
        consts: vec![],
        num_params: 1,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 0,
        }],
        max_stack: 1,
    };
    let outer_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = 1 (x)
            closure(1, 2), // R1 = get
            closure(2, 3), // R2 = set
            load_k(3, 1),  // R3 = 5
            call(2, 2, 1), // set(5)
            call(1, 1, 2), // return get()
            return_(1, 2, 0),
        ],
        consts: vec![Value::Number(1.0), Value::Number(5.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 4,
    };
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = outer
            call(0, 1, 2), // R0 = outer()
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 1,
    };

    let mut vm = Vm::new(vec![main_proto, outer_proto, get_proto, set_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 5.0).abs() < 1e-9));
}

#[test]
fn upvalue_survives_after_outer_return() {
    // Lua 对照：
    //
    //   local function outer()
    //     local x = 1
    //     local function get() return x end
    //     local function set(v) x = v end
    //     return get, set
    //   end
    //
    //   local get, set = outer()
    //   set(99)
    //   return get()
    //
    // 这里验证“closed upvalue”：
    // - outer 返回后，x 对应栈槽位失效
    // - upvalue 会被自动封闭，get/set 仍共享且可持续读写
    let get_proto = Proto {
        code: vec![get_upval(0, 0), return_(0, 2, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 0,
        }],
        max_stack: 1,
    };
    let set_proto = Proto {
        code: vec![set_upval(0, 0), return_(0, 1, 0)],
        consts: vec![],
        num_params: 1,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 0,
        }],
        max_stack: 1,
    };
    let outer_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = 1 (x)
            closure(1, 2), // R1 = get
            closure(2, 3), // R2 = set
            return_(1, 3, 0),
        ],
        consts: vec![Value::Number(1.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = outer
            call(0, 1, 0), // R0,R1 = outer()
            load_k(2, 1),  // R2 = 99
            call(1, 2, 1), // set(99)
            call(0, 1, 2), // return get()
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(99.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    let mut vm = Vm::new(vec![main_proto, outer_proto, get_proto, set_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 99.0).abs() < 1e-9));
}

#[test]
fn nested_upvalue_forwarding_mutable() {
    // Lua 对照（重点是 instack=false 的链式转发仍共享同一单元）：
    //
    //   local function outer()
    //     local x = 5
    //     return function()
    //       local function get() return x end
    //       local function set(v) x = v end
    //       return get, set
    //     end
    //   end
    //
    //   local mid = outer()
    //   local get, set = mid()
    //   set(7)
    //   return get()
    let get_proto = Proto {
        code: vec![get_upval(0, 0), return_(0, 2, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: false,
            index: 0,
        }],
        max_stack: 1,
    };
    let set_proto = Proto {
        code: vec![set_upval(0, 0), return_(0, 1, 0)],
        consts: vec![],
        num_params: 1,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: false,
            index: 0,
        }],
        max_stack: 1,
    };
    let middle_proto = Proto {
        code: vec![
            closure(0, 3), // R0 = get (转发 middle.upvalues[0])
            closure(1, 4), // R1 = set (转发 middle.upvalues[0])
            return_(0, 3, 0),
        ],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 0,
        }],
        max_stack: 2,
    };
    let outer_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = 5
            closure(1, 2), // R1 = middle (捕获 R0)
            return_(1, 2, 0),
        ],
        consts: vec![Value::Number(5.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 2,
    };
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = outer
            call(0, 1, 2), // R0 = outer() => middle
            call(0, 1, 0), // R0,R1 = middle() => get,set
            load_k(2, 1),  // R2 = 7
            call(1, 2, 1), // set(7)
            call(0, 1, 2), // return get()
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1), Value::Number(7.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    let mut vm = Vm::new(vec![
        main_proto,
        outer_proto,
        middle_proto,
        get_proto,
        set_proto,
    ]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 7.0).abs() < 1e-9));
}

#[test]
fn close_instruction_closes_scope_boundary() {
    // Lua 对照（语义对应，不要求语法逐字一致）：
    //
    //   local x, y = 1, 10
    //   local function gx() return x end
    //   local function gy() return y end
    //   close y   -- 语义上等价于关闭 R1 及以上 upvalue
    //   x = 2
    //   y = 20
    //   return gx(), gy()   -- 预期 2, 10
    //
    // 这里验证：
    // - CLOSE 只影响边界及以上（y 被封闭）
    // - 边界以下不受影响（x 仍 open，可看到后续赋值 2）
    let gx_proto = Proto {
        code: vec![get_upval(0, 0), return_(0, 2, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 0,
        }],
        max_stack: 1,
    };
    let gy_proto = Proto {
        code: vec![get_upval(0, 0), return_(0, 2, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 1,
        }],
        max_stack: 1,
    };
    let outer_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = x = 1
            load_k(1, 1),  // R1 = y = 10
            closure(2, 2), // R2 = gx (捕获 R0)
            closure(3, 3), // R3 = gy (捕获 R1)
            close(1),      // 关闭 R1 及以上 upvalue（gy 被封闭，gx 保持 open）
            load_k(0, 2),  // x = 2（gx 应读到 2）
            load_k(1, 3),  // y = 20（gy 应保持 10）
            return_(2, 3, 0),
        ],
        consts: vec![
            Value::Number(1.0),
            Value::Number(10.0),
            Value::Number(2.0),
            Value::Number(20.0),
        ],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 4,
    };
    let main_proto = Proto {
        code: vec![
            load_k(0, 0),  // R0 = outer
            call(0, 1, 0), // R0,R1 = outer() => gx,gy
            call(0, 1, 2), // R0 = gx()
            load_k(1, 0),  // R1 = outer
            call(1, 1, 0), // R1,R2 = outer() => gx,gy
            call(2, 1, 2), // R2 = gy()
            add(0, rk_r(0), rk_r(2)),
            return_(0, 2, 0),
        ],
        consts: vec![Value::LFn(1)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 4,
    };

    let mut vm = Vm::new(vec![main_proto, outer_proto, gx_proto, gy_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 12.0).abs() < 1e-9));
}

#[test]
fn pcall_rollback_with_open_upvalue() {
    // 构造一个“先创建 open upvalue，再触发错误”的函数：
    // - closure(1, 1) 会捕获 R0，产生 open upvalue
    // - add R2 = R1 + R0 会因 Closure + Number 触发 TypeError
    //
    // 这里验证：
    // 1) pcall 失败后 open_upvalues 不残留（避免悬挂索引）
    // 2) VM 仍可继续执行后续调用（状态一致）
    let getter_proto = Proto {
        code: vec![get_upval(0, 0), return_(0, 2, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![UpvalueDesc {
            instack: true,
            index: 0,
        }],
        max_stack: 1,
    };
    let bad_proto = Proto {
        code: vec![
            load_k(0, 0), // R0 = 1
            closure(1, 1),
            add(2, rk_r(1), rk_r(0)),
            return_(2, 2, 0),
        ],
        consts: vec![Value::Number(1.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };
    let ok_proto = Proto {
        code: vec![
            load_k(0, 0),
            load_k(1, 1),
            add(2, rk_r(0), rk_r(1)),
            return_(2, 2, 0),
        ],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
        upvalues: vec![],
        max_stack: 3,
    };

    let mut vm = Vm::new(vec![bad_proto, getter_proto, ok_proto]);
    let bad_func = vm.load(0).unwrap();
    let err = vm.pcall(bad_func, 0, 1).unwrap_err();
    assert!(matches!(err, VmError::TypeError { .. }));
    assert!(vm.open_upvalues.is_empty());

    let ok_func = vm.load(2).unwrap();
    let ret = vm.pcall(ok_func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 3.0).abs() < 1e-9));
}
