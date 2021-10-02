use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    print!("> ");
    io::stdout().flush()?;
    for line in stdin.lock().lines() {
        let line = line?;
        let (command, attr) = line.split_once(' ').unwrap_or((&line, ""));
        if command.is_empty() {
            print!("> ");
            io::stdout().flush()?;
            continue;
        }
        print!("< ");

        match (command, attr) {
            (_, _) => print!("command unknown"),
        }

        print!("\n> ");
        io::stdout().flush()?;
    }
    Ok(())
}
