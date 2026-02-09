use study_lua::opcode::{add, call, load_k, return_, tail_call, vararg};
use study_lua::proto::Proto;

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
        max_stack: 2,
    };
    // f1(): return nothing (B=1 => 0 results)
    let f1_proto = Proto {
        code: vec![return_(0, 1, 0)],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
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
        code: vec![10],
        consts: vec![],
        num_params: 0,
        is_vararg: false,
        max_stack: 1,
    };
    let mut vm = Vm::new(vec![main_proto]);
    let func = vm.load(0).unwrap();

    let err = vm.pcall(func, 0, 1).unwrap_err();
    match err {
        VmError::UnknownOpcode(10) => {}
        other => panic!("expected UnknownOpcode(10), got {other:?}"),
    }
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
        max_stack: 3,
    };

    // protos: [main, g, f]
    let mut vm = Vm::new(vec![main_proto, g_proto, f_proto]);
    let func = vm.load(0).unwrap();
    let ret = vm.pcall(func, 0, 1).unwrap();
    assert!(matches!(ret, Value::Number(n) if (n - 3.0).abs() < 1e-9));
}

#[test]
fn nested_call_via_call_opcode() {
    // f(): return 1
    let f_proto = Proto {
        code: vec![load_k(0, 0), return_(0, 0, 0)],
        consts: vec![Value::Number(1.0)],
        num_params: 0,
        is_vararg: false,
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
fn call_c0_multi_returns_and_updates_top() {
    // f(): return 1, 2  (Return B=3 => 2 results)
    let f_proto = Proto {
        code: vec![load_k(0, 0), load_k(1, 1), return_(0, 3, 0)],
        consts: vec![Value::Number(1.0), Value::Number(2.0)],
        num_params: 0,
        is_vararg: false,
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
        max_stack: 2,
    };

    // g(a, b): return a + b
    let g_proto = Proto {
        code: vec![add(0, rk_r(0), rk_r(1)), return_(0, 2, 0)],
        consts: vec![],
        num_params: 2,
        is_vararg: false,
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
