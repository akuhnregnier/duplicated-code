use clap::Parser;
use std::fs;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    file: std::path::PathBuf,
}

fn main() {
    let args = Args::parse();
    println!("In file {:?}", args.file);
    println!("In file {:?}", args.file.canonicalize());

    let Ok(file_path) = args.file.canonicalize() else {
        panic!("Invalid file '{:?}'", args.file)
    };

    let contents = fs::read_to_string(file_path).expect("Unable to read file.");

    for line in contents.lines().map(|line| line.trim()) {
        println!("Line:{line}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_result() {
        assert_eq!(1, 1);
    }
}
