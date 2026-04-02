use rusqlite::{Connection, Result};
use std::io::{self, Write};



mod core;
mod console;

#[tokio::main]
async fn main() -> Result<()>  {
    let db_path = String::from("users.db");

    let mut conn = Connection::open(&db_path)?;
    core::create_tables(&mut conn)?;
    core::sync_next_payment_dates(&conn)?;
    core::spawn_midnight_days_left_worker(db_path);

    loop {
        // println!("input command");
        // print!("\x1b[34m>> \x1b[0m ");
        print!("\x1b[34m>> \x1b[0m ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).expect("Failed to read line");
        let input = input.trim();
        match input.split_whitespace().collect::<Vec<&str>>().as_slice()  {
            ["adduser", name] => {
                core::add_user(&mut conn, name).await?;
            },
            ["adddays", name, days] => {
                let days = days.parse::<i32>().unwrap_or(0);
                core::add_days(&mut conn, name, days)?;
            },
            [""] => continue,
            ["quit"] | ["exit"] => break,
            _ => {
                println!("\x1b[1m\x1b[31mUnknown command: \x1b[34m{}\x1b[0m", input);
                println!("{}", "Type '\x1b[34mhelp\x1b[0m' for a list of available commands.");
            }
        }
    }

    Ok(())
}
