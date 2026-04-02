use chrono::{Duration as ChronoDuration, Local, TimeZone, Utc};
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
use std::thread;
use std::time::Duration;
use crate::console::{color_fmt_log, color_fmt_err, color_fmt_ok};
use reqwest::Client;
use serde_json::json;


static ADDR: &str = "http://localhost:41121/y7PTrUr7ENnjFltjtT";





pub fn create_tables(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "CREATE TABLE IF NOT EXISTS user_auth (
            uuid TEXT PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            days_left INTEGER NOT NULL DEFAULT 0,
            total_GB FLOAT NOT NULL DEFAULT 0,
            next_payment_date DATE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            is_active BOOLEAN NOT NULL DEFAULT TRUE
        )",
        [],
    )?;
    tx.execute(
        "CREATE TRIGGER IF NOT EXISTS user_auth_sync_next_payment_after_insert
        AFTER INSERT ON user_auth
        BEGIN
            UPDATE user_auth
            SET next_payment_date = date('now', printf('+%d days', NEW.days_left - 1))
            WHERE uuid = NEW.uuid;
        END;",
        [],
    )?;
    tx.execute(
        "CREATE TRIGGER IF NOT EXISTS user_auth_sync_next_payment_after_update
        AFTER UPDATE OF days_left ON user_auth
        BEGIN
            UPDATE user_auth
            SET next_payment_date = date('now', printf('+%d days', NEW.days_left - 1))
            WHERE uuid = NEW.uuid;
        END;",
        [],
    )?;
    tx.commit()?;
    Ok(())
}

pub async fn add_user(conn: &mut Connection, name: &str) -> Result<()> {
    let uuid = Uuid::new_v4().to_string();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO user_auth (uuid, name) VALUES (?1, ?2)",
        params![uuid, name],
    )?;
    tx.commit()?;
    println!("{}", color_fmt_ok("Successfully added user: {}", &[name]));
    println!("{}", color_fmt_log("trying to add to panel: {}", &[name]));
    let _ = add_to_panel(uuid.as_str(), name, 30, &[1, 2]).await;

    Ok(())

}


pub fn add_days(conn: &mut Connection, name: &str, days: i32) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE user_auth SET days_left = days_left + ?1 WHERE name = ?2",
        params![days, name],
    )?;
    tx.commit()?;
    println!("{}", color_fmt_ok("Successfully added days: {} to user: {}", &[days.to_string().as_str(), name]));
    Ok(())

}


pub async fn add_to_panel(uuid: &str, email: &str, days: i64, inb: &[i8]) -> Result<(), String> {
    for inbnum in inb {
        let client = Client::builder()
        .cookie_store(true)
        .build()
        .map_err(|e| e.to_string())?;


    let login_res = client
        .post(format!("{}/login", ADDR))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("username=admin&password=admin")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    println!("{}", color_fmt_log("Trying to login", &[]));
    if !login_res.status().is_success() {
        return Err("login failed".into());
    }


    let now = Utc::now().date_naive();
    let target_date = now + ChronoDuration::days(days);
    let target_dt = Utc
        .from_utc_datetime(&target_date.and_hms_opt(0, 0, 0).unwrap());

    let expiry_ms = target_dt.timestamp_millis();

    let settings = format!(
        "{{\"clients\":[{{\"id\":\"{}\",\"email\":\"{}-{}\",\"enable\":true,\"expiryTime\":{}}}]}}",
        uuid, email,inbnum , expiry_ms
    );

    
        let body = json!({
        "id": inbnum,
        "settings": settings
    });

    println!("{}", color_fmt_log("Trying to add client to panel", &[]));
    let res = client
        .post(format!("{}/panel/api/inbounds/addClient", ADDR))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    if !res.status().is_success() {
        return Err(format!("addClient failed: {}", res.status()));
    }
    }
    println!("{}", color_fmt_ok("Successfully added user to panel: {}", &[email]));
    Ok(())
}








pub fn sync_next_payment_dates(conn: &Connection) -> Result<usize> {
    conn.execute(
        "UPDATE user_auth
         SET next_payment_date = date('now', printf('+%d days', days_left - 1))",
        [],
    )
}

pub fn decrement_days_left(conn: &Connection) -> Result<usize> {
    conn.execute(
        "UPDATE user_auth
         SET days_left = CASE
             WHEN days_left > 0 THEN days_left - 1
             ELSE 0
         END",
        [],
    )
}

pub fn spawn_midnight_days_left_worker(db_path: String) {
    thread::spawn(move || loop {
        let sleep_duration = duration_until_next_midnight();
        thread::sleep(sleep_duration);

        match Connection::open(&db_path) {
            Ok(conn) => {
                if let Err(error) = decrement_days_left(&conn) {
                    eprintln!("Failed to decrement days_left: {error}");
                }
                if let Err(error) = sync_next_payment_dates(&conn) {
                    eprintln!("Failed to sync next_payment_date: {error}");
                }
            }
            Err(error) => eprintln!("Failed to open database for scheduled update: {error}"),
        }
    });
}

fn duration_until_next_midnight() -> Duration {
    let now = Local::now();
    let next_day = now.date_naive() + ChronoDuration::days(1);
    let next_midnight_naive = next_day.and_hms_opt(0, 0, 0).unwrap();

    let next_midnight = Local
        .from_local_datetime(&next_midnight_naive)
        .single()
        .unwrap_or(now + ChronoDuration::days(1));

    next_midnight
        .signed_duration_since(now)
        .to_std()
        .unwrap_or_else(|_| Duration::from_secs(60))
}

