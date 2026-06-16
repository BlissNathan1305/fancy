use clap::{Parser, ValueEnum};

/// A simple and safe CLI calculator written in Rust.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The first number
    num1: f64,

    /// The operation to perform
    #[arg(value_enum)]
    operator: Operator,

    /// The second number
    num2: f64,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum Operator {
    /// Addition (+)
    Add,
    /// Subtraction (-)
    Sub,
    /// Multiplication (*)
    Mul,
    /// Division (/)
    Div,
}

fn main() {
    // Parse the command line arguments
    let args = Args::parse();

    // Perform the calculation based on the operator
    match args.operator {
        Operator::Add => {
            println!("Result: {}", args.num1 + args.num2);
        }
        Operator::Sub => {
            println!("Result: {}", args.num1 - args.num2);
        }
        Operator::Mul => {
            println!("Result: {}", args.num1 * args.num2);
        }
        Operator::Div => {
            // Guard against division by zero
            if args.num2 == 0.0 {
                eprintln!("Error: Division by zero is undefined.");
                std::process::exit(1);
            }
            println!("Result: {}", args.num1 / args.num2);
        }
    }
}
