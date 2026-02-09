use std::path::Path;
use study_lua::compile_file;
use study_lua::vm::Vm;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: cargo run -- <script.lua>");
        std::process::exit(2);
    }

    let script_path = Path::new(&args[1]);
    let protos = match compile_file(script_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("compile error: {}", e);
            std::process::exit(1);
        }
    };
    let mut vm = Vm::new(protos);

    let func = match vm.load(0) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("vm load error: {}", e);
            std::process::exit(1);
        }
    };

    match vm.pcall(func, 0, 1) {
        Ok(ret) => println!("ret = {:?}", ret),
        Err(e) => {
            eprintln!("runtime error: {}", e);
            std::process::exit(1);
        }
    }
}
