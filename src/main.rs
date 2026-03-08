use std::io::Read;
use std::path::Path;

use baselang::{eval, observe, parser};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file_path = args.get(1).map(Path::new);

    let source = if let Some(path) = file_path {
        std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("error reading {}: {e}", path.display());
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

    match eval::eval_with_metrics(&stmts) {
        Ok(artifacts) => {
            persist_observability(file_path, &source, &artifacts.metrics);
            for line in artifacts.output {
                println!("{line}");
            }
        }
        Err(failure) => {
            persist_observability(file_path, &source, &failure.metrics);
            eprintln!("runtime error: {}", failure.error.message);
            std::process::exit(1);
        }
    }
}

fn persist_observability(file_path: Option<&Path>, source: &str, metrics: &eval::ExecutionMetrics) {
    let Some(file_path) = file_path else {
        return;
    };

    let report = observe::build_report(source, metrics);
    if let Err(err) = observe::write_report(file_path, &report) {
        eprintln!(
            "warning: failed to write {}: {err}",
            observe::observability_path(file_path).display()
        );
    }
}

extern "C" {
    #[link_name = "isatty"]
    fn libc_isatty(fd: i32) -> i32;
}
