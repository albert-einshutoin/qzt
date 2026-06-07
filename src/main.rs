use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args();
    let _program = args.next();

    match args.next().as_deref() {
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("--version") | Some("-V") => {
            println!("{}", qzt::version());
            ExitCode::SUCCESS
        }
        Some(command) => {
            eprintln!("qzt: unknown command '{command}'");
            eprintln!("try 'qzt --help'");
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!("qzt {}", qzt::version());
    println!();
    println!("Usage: qzt <COMMAND>");
    println!();
    println!("Commands:");
    println!("  help       Show this help");
    println!();
    println!("Options:");
    println!("  -h, --help     Show this help");
    println!("  -V, --version  Show version");
}
