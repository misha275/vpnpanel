use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
// use std::thread;
// use std::time::Duration;
use crate::console::{color_fmt_log, color_fmt_err, color_fmt_ok};
use reqwest::Client;
use serde_json::{json, Value};
use std::io::{Write};


static ADDR: &str = "http://localhost:41121/y7PTrUr7ENnjFltjtT";
static INBLIST: &[i8] = &[1, 2];





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
    let _ = add_to_panel(uuid.as_str(), name, 30, INBLIST).await;

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

    println!("{}", color_fmt_log("Trying to add client to panel id: {}", &[&inbnum.to_string()]));
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


pub async fn add_days(conn: &mut Connection, name: &str, days: i32) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE user_auth SET days_left = days_left + ?1 WHERE name = ?2",
        params![days, name],
    )?;
    tx.commit()?;

    let uuid: String = conn.query_row(
        "SELECT uuid FROM user_auth WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?;

    let dbdays: i64 = conn.query_row(
        "SELECT days_left FROM user_auth WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?;
    println!("{}", color_fmt_ok("Successfully added days: {} to user: {}", &[days.to_string().as_str(), name]));
    let _ = extend_user(&uuid, name, dbdays, INBLIST).await;
    Ok(())

}


pub async fn extend_user(
    uuid: &str,
    email: &str,
    dbdays: i64,
    inb: &[i8],
) -> Result<(), String> {

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

    for inbnum in inb {

        let now = Utc::now().date_naive();
        let target = now + ChronoDuration::days(dbdays);
        let dt = Utc.from_utc_datetime(&target.and_hms_opt(0,0,0).unwrap());
        let expiry = dt.timestamp_millis();

        let settings = format!(
            "{{\"clients\":[{{\
                \"id\":\"{}\",\
                \"flow\":\"\",\
                \"email\":\"{}-{}\",\
                \"limitIp\":0,\
                \"totalGB\":0,\
                \"expiryTime\":{},\
                \"enable\":true,\
                \"tgId\":\"\",\
                \"subId\":\"\",\
                \"comment\":\"\",\
                \"reset\":0\
            }}]}}",
            uuid, email, inbnum, expiry
        );

        println!("{}", color_fmt_log("Updating client", &[]));

        let res = client
            .post(format!(
                "{}/panel/api/inbounds/updateClient/{}",
                ADDR, uuid
            ))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("id={}&settings={}", inbnum, settings))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let text = res.text().await.unwrap_or_default();
        println!("RESPONSE: {}", text);

        // if !res.status().is_success() {
        //     return Err(format!("updateClient failed: {}", res.status()));
        // }
    }

    println!("{}", color_fmt_ok("Extended user: {}", &[email]));
    Ok(())
}







pub async fn change_status(conn: &mut Connection, name: &str, status: bool) -> Result<()> {
    let uuid: String = conn.query_row(
        "SELECT uuid FROM user_auth WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?;
    println!("{}", color_fmt_ok("Successfully changed status for user: {}, status: {}", &[name, status.to_string().as_str()]));
    let _ = change_status_api(&uuid, name, status, INBLIST).await;
    Ok(())

}


pub async fn change_status_api(
    uuid: &str,
    email: &str,
    status: bool,
    inb: &[i8],
) -> Result<(), String> {

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

    if !login_res.status().is_success() {
        return Err("login failed".into());
    }

    for inbnum in inb {

        let settings = format!(
            "{{\"clients\":[{{\"id\":\"{}\",\"email\":\"{}-{}\",\"enable\":{}}}]}}",
            uuid, email, inbnum, status
        );

        let res = client
            .post(format!(
                "{}/panel/api/inbounds/updateClient/{}",
                ADDR, uuid
            ))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("id={}&settings={}", inbnum, settings))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let text = res.text().await.unwrap_or_default();
        println!("RESPONSE: {}", text);
    }

    Ok(())
}


pub fn help() {
    println!("{}", color_fmt_log("Available commands:", &[]));
    println!("{}", color_fmt_log("adduser <name> - Add a new user", &[]));
    println!("{}", color_fmt_log("adddays <uuid> <name> <days> - Add days to a user", &[]));
    println!("{}", color_fmt_log("sync - Sync with API", &[]));
    println!("{}", color_fmt_log("help - Show this message", &[]));
    println!("{}", color_fmt_log("quit/exit - Exit the program", &[]));
}

pub trait TrimNameSuffix {
    fn trim_name_suffix(&self) -> String;
}

impl TrimNameSuffix for str {
    fn trim_name_suffix(&self) -> String {
        self.split_once('-')
            .map(|(base, _)| base.to_string())
            .unwrap_or_else(|| self.to_string())
    }
}

pub async fn sync_db(db_path: &str) -> Result<(), String> {

    let client = Client::builder()
        .cookie_store(true)
        .build()
        .map_err(|e| e.to_string())?;


    client.post(format!("{}/login", ADDR))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("username=admin&password=admin")
        .send()
        .await
        .map_err(|e| e.to_string())?;


    let res = client
        .get(format!("{}/panel/api/inbounds/list", ADDR))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let text = res.text().await.map_err(|e| e.to_string())?;
    let data: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    let now = Utc::now();

    for inbound in data["obj"].as_array().ok_or("no obj")? {
        for c in inbound["clientStats"].as_array().ok_or("no clientStats")? {

            let uuid = c["uuid"].as_str().unwrap_or("");
            let email = c["email"].as_str().unwrap_or("").trim_name_suffix();

            let enable = c["enable"].as_bool().unwrap_or(false);

            let up = c["up"].as_i64().unwrap_or(0);
            let down = c["down"].as_i64().unwrap_or(0);
            let total_gb = (up + down) as f64 / 1024f64 / 1024f64 / 1024f64;

            let expiry = c["expiryTime"].as_i64().unwrap_or(0);

            let (days_left, next_date) = if expiry > 0 {
                let dt = Utc.timestamp_millis_opt(expiry).unwrap();
                let diff = dt - now;
                let days = diff.num_days();

                (days, dt.date_naive().to_string())
            } else {
                (0, "".to_string())
            };

            conn.execute(
                "INSERT INTO user_auth (uuid, name, days_left, total_GB, next_payment_date, is_active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(uuid) DO UPDATE SET
                    name=excluded.name,
                    days_left=excluded.days_left,
                    total_GB=excluded.total_GB,
                    next_payment_date=excluded.next_payment_date,
                    is_active=excluded.is_active",
                params![
                    uuid,
                    email,
                    days_left,
                    total_gb,
                    next_date,
                    enable
                ]
            ).map_err(|e| e.to_string())?;
        }
    }

    println!("\r\x1b[2K{}", color_fmt_ok("DB synced", &[]));
    std::io::stdout().flush().unwrap();
    print!("\r\x1b[2K\x1b[34m>> \x1b[0m");
    std::io::stdout().flush().unwrap();
    Ok(())
}


pub fn spawn_minute_sync_worker(db_path: String) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            ticker.tick().await;
            if let Err(error) = sync_db(&db_path).await {
                eprintln!("Failed to sync db: {error}");
            }
        }
    });
}