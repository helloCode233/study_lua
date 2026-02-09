use study_lua::compile_str;
use study_lua::vm::Vm;
use study_lua::{CompileError, Value};

fn run_script(src: &str) -> Value {
    let protos = compile_str(src).unwrap();
    let mut vm = Vm::new(protos);
    let func = vm.load(0).unwrap();
    vm.pcall(func, 0, 1).unwrap()
}

#[test]
fn compile_and_run_add_mul() {
    let ret = run_script("return 1 + 2 * 3");
    assert!(matches!(ret, Value::Number(n) if (n - 7.0).abs() < 1e-9));
}

#[test]
fn compile_and_run_unary_minus_and_sub() {
    let ret = run_script("local x = 10 return -x + 4");
    assert!(matches!(ret, Value::Number(n) if (n + 6.0).abs() < 1e-9));
}

#[test]
fn compile_and_run_div() {
    let ret = run_script("return 8 / 2");
    assert!(matches!(ret, Value::Number(n) if (n - 4.0).abs() < 1e-9));
}

#[test]
fn compile_and_run_local_function_call() {
    let script = r#"
local function f(a)
  return a * 2
end
return f(3)
"#;
    let ret = run_script(script);
    assert!(matches!(ret, Value::Number(n) if (n - 6.0).abs() < 1e-9));
}

#[test]
fn compile_and_run_nested_closure_capture() {
    let script = r#"
local function outer()
  local x = 21
  local function inner()
    return x * 2
  end
  return inner()
end
return outer()
"#;
    let ret = run_script(script);
    assert!(matches!(ret, Value::Number(n) if (n - 42.0).abs() < 1e-9));
}

#[test]
fn compile_and_run_if_truthiness() {
    let ret_true = run_script("local x = 0 if x then return 1 else return 2 end");
    assert!(matches!(ret_true, Value::Number(n) if (n - 1.0).abs() < 1e-9));

    let ret_false = run_script("if nil then return 1 else return 2 end");
    assert!(matches!(ret_false, Value::Number(n) if (n - 2.0).abs() < 1e-9));
}

#[test]
fn compile_rejects_global_function_definition() {
    let script = r#"
function f(a)
  return a
end
return f(1)
"#;
    let err = match compile_str(script) {
        Ok(_) => panic!("expected compile error"),
        Err(e) => e,
    };
    match err {
        CompileError::Unsupported { feature, .. } => {
            assert!(feature.contains("global function definition"));
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}
