use std::io::Read;

use baselang::{eval, parser};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let source = if args.len() > 1 {
        std::fs::read_to_string(&args[1]).unwrap_or_else(|e| {
            eprintln!("error reading {}: {e}", args[1]);
            std::process::exit(1);
        })
    } else {
        let is_tty = unsafe { libc_isatty(0) != 0 };
        if is_tty {
            eprintln!("baselang - enter your program, then Ctrl+D to run");
        }
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).unwrap();
        buf
    };

    let stmts = match parser::parse(&source) {
        Ok(stmts) => stmts,
        Err(e) => {
            eprintln!("parse error: {}", e.message);
            std::process::exit(1);
        }
    };

    match eval::eval(&stmts) {
        Ok(output) => {
            for line in output {
                println!("{line}");
            }
        }
        Err(e) => {
            eprintln!("runtime error: {}", e.message);
            std::process::exit(1);
        }
    }
}

extern "C" {
    #[link_name = "isatty"]
    fn libc_isatty(fd: i32) -> i32;
}
