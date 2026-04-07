use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
// use std::thread;
// use std::time::Duration;
use crate::console::{color_fmt_err, color_fmt_log, color_fmt_ok};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Write};


static ADDR: &str = "http://localhost:41121/y7PTrUr7ENnjFltjtT";
static INBLIST: &[i8] = &[1, 2];




pub fn help() {
    println!("{}", color_fmt_log("Available commands:", &[]));
    println!("{}", color_fmt_log("adduser {} - Add a new user", &["<name>"]));
    println!("{}", color_fmt_log("adddays {} {} - Add days to a user", &["<name>", "<days>"]));
    println!("{}", color_fmt_log("changestatus {} {} - Change user status", &["<name>", "<true/false>"]));
    println!("{}", color_fmt_log("sync - Sync with API", &[]));
    println!("{}", color_fmt_log("help - Show this message", &[]));
    println!("{}", color_fmt_log("quit/exit - Exit the program", &[]));
}

#[derive(Debug)]
struct SyncedUser {
    name: String,
    is_active: bool,
    max_expiry: i64,
    up: HashMap<i64, i64>,
    down: HashMap<i64, i64>,
    total_gb: HashMap<i64, f64>,
    last_online: HashMap<i64, i64>,
}

fn configured_inbound_ids() -> Vec<i64> {
    INBLIST.iter().map(|value| i64::from(*value)).collect()
}

fn first_inbound_id() -> Option<i64> {
    INBLIST.first().map(|value| i64::from(*value))
}

fn value_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|number| number as i64))
        .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
}

fn value_to_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|number| number as f64))
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
}

fn ordered_inbound_ids_i64(metric: &HashMap<i64, i64>) -> Vec<i64> {
    let mut ordered = configured_inbound_ids();
    let mut extras: Vec<i64> = metric
        .keys()
        .copied()
        .filter(|id| !ordered.contains(id))
        .collect();
    extras.sort_unstable();
    ordered.extend(extras);
    ordered
}

fn ordered_inbound_ids_f64(metric: &HashMap<i64, f64>) -> Vec<i64> {
    let mut ordered = configured_inbound_ids();
    let mut extras: Vec<i64> = metric
        .keys()
        .copied()
        .filter(|id| !ordered.contains(id))
        .collect();
    extras.sort_unstable();
    ordered.extend(extras);
    ordered
}

fn metric_i64_to_json(metric: &HashMap<i64, i64>) -> String {
    let rows: Vec<Value> = ordered_inbound_ids_i64(metric)
        .into_iter()
        .map(|inb_id| {
            json!({
                "inbID": inb_id,
                "data": metric.get(&inb_id).copied().unwrap_or(0)
            })
        })
        .collect();
    Value::Array(rows).to_string()
}

fn metric_f64_to_json(metric: &HashMap<i64, f64>) -> String {
    let rows: Vec<Value> = ordered_inbound_ids_f64(metric)
        .into_iter()
        .map(|inb_id| {
            json!({
                "inbID": inb_id,
                "data": metric.get(&inb_id).copied().unwrap_or(0.0)
            })
        })
        .collect();
    Value::Array(rows).to_string()
}

fn parse_i64_metric_from_text(text: &str) -> HashMap<i64, i64> {
    let mut metric = HashMap::new();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return metric;
    }

    if let Ok(json_value) = serde_json::from_str::<Value>(trimmed) {
        match json_value {
            Value::Array(items) => {
                let mut parsed_rows = false;

                for item in &items {
                    if let Some(obj) = item.as_object() {
                        let inb_id = obj
                            .get("inbID")
                            .and_then(value_to_i64)
                            .or_else(|| obj.get("inbound_id").and_then(value_to_i64))
                            .or_else(|| obj.get("id").and_then(value_to_i64));
                        let data = obj
                            .get("data")
                            .and_then(value_to_i64)
                            .or_else(|| obj.get("value").and_then(value_to_i64));

                        if let (Some(inb_id), Some(data)) = (inb_id, data) {
                            metric.insert(inb_id, data);
                            parsed_rows = true;
                        }
                    }
                }

                if parsed_rows {
                    return metric;
                }

                for (index, item) in items.iter().enumerate() {
                    if let Some(data) = value_to_i64(item) {
                        if let Some(inb_id) = INBLIST.get(index) {
                            metric.insert(i64::from(*inb_id), data);
                        }
                    }
                }

                if !metric.is_empty() {
                    return metric;
                }
            }
            Value::Object(object) => {
                for (key, value) in object {
                    if let (Ok(inb_id), Some(data)) = (key.parse::<i64>(), value_to_i64(&value)) {
                        metric.insert(inb_id, data);
                    }
                }

                if !metric.is_empty() {
                    return metric;
                }
            }
            other => {
                if let Some(data) = value_to_i64(&other) {
                    if let Some(inb_id) = first_inbound_id() {
                        metric.insert(inb_id, data);
                    }
                    return metric;
                }
            }
        }
    }

    if let Ok(data) = trimmed.parse::<i64>() {
        if let Some(inb_id) = first_inbound_id() {
            metric.insert(inb_id, data);
        }
        return metric;
    }

    if let Ok(data) = trimmed.parse::<f64>() {
        if let Some(inb_id) = first_inbound_id() {
            metric.insert(inb_id, data as i64);
        }
    }

    metric
}

fn parse_f64_metric_from_text(text: &str) -> HashMap<i64, f64> {
    let mut metric = HashMap::new();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return metric;
    }

    if let Ok(json_value) = serde_json::from_str::<Value>(trimmed) {
        match json_value {
            Value::Array(items) => {
                let mut parsed_rows = false;

                for item in &items {
                    if let Some(obj) = item.as_object() {
                        let inb_id = obj
                            .get("inbID")
                            .and_then(value_to_i64)
                            .or_else(|| obj.get("inbound_id").and_then(value_to_i64))
                            .or_else(|| obj.get("id").and_then(value_to_i64));
                        let data = obj
                            .get("data")
                            .and_then(value_to_f64)
                            .or_else(|| obj.get("value").and_then(value_to_f64));

                        if let (Some(inb_id), Some(data)) = (inb_id, data) {
                            metric.insert(inb_id, data);
                            parsed_rows = true;
                        }
                    }
                }

                if parsed_rows {
                    return metric;
                }

                for (index, item) in items.iter().enumerate() {
                    if let Some(data) = value_to_f64(item) {
                        if let Some(inb_id) = INBLIST.get(index) {
                            metric.insert(i64::from(*inb_id), data);
                        }
                    }
                }

                if !metric.is_empty() {
                    return metric;
                }
            }
            Value::Object(object) => {
                for (key, value) in object {
                    if let (Ok(inb_id), Some(data)) = (key.parse::<i64>(), value_to_f64(&value)) {
                        metric.insert(inb_id, data);
                    }
                }

                if !metric.is_empty() {
                    return metric;
                }
            }
            other => {
                if let Some(data) = value_to_f64(&other) {
                    if let Some(inb_id) = first_inbound_id() {
                        metric.insert(inb_id, data);
                    }
                    return metric;
                }
            }
        }
    }

    if let Ok(data) = trimmed.parse::<f64>() {
        if let Some(inb_id) = first_inbound_id() {
            metric.insert(inb_id, data);
        }
        return metric;
    }

    if let Ok(data) = trimmed.parse::<i64>() {
        if let Some(inb_id) = first_inbound_id() {
            metric.insert(inb_id, data as f64);
        }
    }

    metric
}

fn parse_i64_metric_from_sql(value: SqlValue) -> HashMap<i64, i64> {
    match value {
        SqlValue::Integer(number) => {
            let mut metric = HashMap::new();
            if let Some(inb_id) = first_inbound_id() {
                metric.insert(inb_id, number);
            }
            metric
        }
        SqlValue::Real(number) => {
            let mut metric = HashMap::new();
            if let Some(inb_id) = first_inbound_id() {
                metric.insert(inb_id, number as i64);
            }
            metric
        }
        SqlValue::Text(text) => parse_i64_metric_from_text(&text),
        _ => HashMap::new(),
    }
}

fn parse_f64_metric_from_sql(value: SqlValue) -> HashMap<i64, f64> {
    match value {
        SqlValue::Integer(number) => {
            let mut metric = HashMap::new();
            if let Some(inb_id) = first_inbound_id() {
                metric.insert(inb_id, number as f64);
            }
            metric
        }
        SqlValue::Real(number) => {
            let mut metric = HashMap::new();
            if let Some(inb_id) = first_inbound_id() {
                metric.insert(inb_id, number);
            }
            metric
        }
        SqlValue::Text(text) => parse_f64_metric_from_text(&text),
        _ => HashMap::new(),
    }
}

fn get_i64_metric_table(conn: &Connection, name: &str, column: &str) -> Result<HashMap<i64, i64>> {
    let sql = format!("SELECT {} FROM user_auth WHERE name = ?1", column);
    conn.query_row(&sql, params![name], |row| {
        let value: SqlValue = row.get(0)?;
        Ok(parse_i64_metric_from_sql(value))
    })
}

fn empty_i64_metric_json() -> String {
    let metric: HashMap<i64, i64> = HashMap::new();
    metric_i64_to_json(&metric)
}

fn empty_f64_metric_json() -> String {
    let metric: HashMap<i64, f64> = HashMap::new();
    metric_f64_to_json(&metric)
}





pub fn create_tables(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "CREATE TABLE IF NOT EXISTS user_auth (
            uuid TEXT PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            days_left INTEGER NOT NULL DEFAULT 0,
            up TEXT NOT NULL DEFAULT '[]',
            down TEXT NOT NULL DEFAULT '[]',
            total_GB TEXT NOT NULL DEFAULT '[]',
            next_payment_date DATE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            last_online TEXT NOT NULL DEFAULT '[]',
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

    let mut normalized_rows = Vec::new();
    {
        let mut stmt = tx.prepare("SELECT uuid, up, down, total_GB, last_online FROM user_auth")?;
        let rows = stmt.query_map([], |row| {
            let uuid: String = row.get(0)?;
            let up: SqlValue = row.get(1)?;
            let down: SqlValue = row.get(2)?;
            let total_gb: SqlValue = row.get(3)?;
            let last_online: SqlValue = row.get(4)?;

            Ok((
                uuid,
                metric_i64_to_json(&parse_i64_metric_from_sql(up)),
                metric_i64_to_json(&parse_i64_metric_from_sql(down)),
                metric_f64_to_json(&parse_f64_metric_from_sql(total_gb)),
                metric_i64_to_json(&parse_i64_metric_from_sql(last_online)),
            ))
        })?;

        for row in rows {
            normalized_rows.push(row?);
        }
    }

    for (uuid, up_json, down_json, total_gb_json, last_online_json) in normalized_rows {
        tx.execute(
            "UPDATE user_auth SET up = ?1, down = ?2, total_GB = ?3, last_online = ?4 WHERE uuid = ?5",
            params![up_json, down_json, total_gb_json, last_online_json, uuid],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub async fn add_user(conn: &mut Connection, name: &str) -> Result<()> {
    let uuid = Uuid::new_v4().to_string();
    let up_json = empty_i64_metric_json();
    let down_json = empty_i64_metric_json();
    let total_gb_json = empty_f64_metric_json();
    let last_online_json = empty_i64_metric_json();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO user_auth (uuid, name, up, down, total_GB, last_online) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![uuid, name, up_json, down_json, total_gb_json, last_online_json],
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
    // let tx = conn.transaction()?;
    // tx.execute(
    //     "UPDATE user_auth SET days_left = days_left + ?1 WHERE name = ?2",
    //     params![days, name],
    // )?;
    // tx.commit()?;

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
    let newdays: i64 = dbdays + days as i64;
    let up = get_i64_metric_table(conn, name, "up")?;
    let down = get_i64_metric_table(conn, name, "down")?;
    let enable: bool = conn.query_row(
        "SELECT is_active FROM user_auth WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?;
    let lastonline = get_i64_metric_table(conn, name, "last_online")?;
    println!("{}", color_fmt_ok("Successfully added days: {} to user: {}", &[days.to_string().as_str(), name]));
    let _ = extend_user(&uuid, name, newdays, INBLIST, enable, &up, &down, &lastonline).await;
    Ok(())

}

pub async fn extend_user(
    uuid: &str,
    email: &str,
    dbdays: i64,
    inb: &[i8],
    enable: bool,
    up: &HashMap<i64, i64>,
    down: &HashMap<i64, i64>,
    lastonline: &HashMap<i64, i64>,
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

    let mut failed_updates: Vec<String> = Vec::new();

    for inbnum in inb {
        let inb_id = i64::from(*inbnum);
        let inbound_up = up.get(&inb_id).copied().unwrap_or(0);
        let inbound_down = down.get(&inb_id).copied().unwrap_or(0);
        let inbound_last_online = lastonline.get(&inb_id).copied().unwrap_or(0);

        let now = Utc::now().date_naive();
        let target = now + ChronoDuration::days(dbdays);
        let dt = Utc.from_utc_datetime(&target.and_hms_opt(0,0,0).unwrap());
        let expiry = dt.timestamp_millis();

        let settings = json!({
            "clients": [
                {
                    "enable": enable,
                    "email": format!("{}-{}", email, inbnum),
                    "id": uuid,
                    "up": inbound_up,
                    "down": inbound_down,
                    "expiryTime": expiry,
                    "lastOnline": inbound_last_online
                }
            ]
        })
        .to_string();

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

        let status = res.status();

        let text = res.text().await.unwrap_or_default();
        println!("RESPONSE: {}", text);

        let parsed = serde_json::from_str::<Value>(&text).ok();
        let api_success = parsed
            .as_ref()
            .and_then(|payload| payload["success"].as_bool())
            .unwrap_or(false);

        if !status.is_success() || !api_success {
            let message = parsed
                .as_ref()
                .and_then(|payload| payload["msg"].as_str())
                .unwrap_or("updateClient failed")
                .to_string();

            failed_updates.push(format!(
                "inbound {}: {} (status {})",
                inbnum, message, status
            ));
        }
    }

    if failed_updates.is_empty() {
        println!("{}", color_fmt_ok("Extended user: {}", &[email]));
        Ok(())
    } else {
        let details = failed_updates.join("; ");
        println!(
            "{}",
            color_fmt_err(
                "Failed to extend user: {} ({})",
                &[email, details.as_str()]
            )
        );
        Err(format!("failed to extend user {}: {}", email, details))
    }
}


pub async fn change_status(conn: &mut Connection, name: &str, status: bool) -> Result<()> {
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
    let up = get_i64_metric_table(conn, name, "up")?;
    let down = get_i64_metric_table(conn, name, "down")?;
    let lastonline = get_i64_metric_table(conn, name, "last_online")?;
    println!("{}", color_fmt_ok("Successfully changed status for user: {}, status: {}", &[name, status.to_string().as_str()]));
    // let _ = change_status_api(&uuid, name, status, INBLIST).await;
    let _ = extend_user(&uuid, name, dbdays, INBLIST, status, &up, &down, &lastonline).await;
    Ok(())

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
    let mut users: HashMap<String, SyncedUser> = HashMap::new();

    for inbound in data["obj"].as_array().ok_or("no obj")? {
        let Some(inbound_id) = inbound["id"].as_i64() else {
            continue;
        };

        for c in inbound["clientStats"].as_array().ok_or("no clientStats")? {

            let uuid = c["uuid"].as_str().unwrap_or("");
            if uuid.is_empty() {
                continue;
            }

            let email = c["email"].as_str().unwrap_or("").trim_name_suffix();
            let enable = c["enable"].as_bool().unwrap_or(false);

            let up = c["up"].as_i64().unwrap_or(0);
            let down = c["down"].as_i64().unwrap_or(0);
            let total_gb = (up + down) as f64;
            let last_online = c["lastOnline"].as_i64().unwrap_or(0);
            let expiry = c["expiryTime"].as_i64().unwrap_or(0);

            let entry = users.entry(uuid.to_string()).or_insert_with(|| SyncedUser {
                name: email.clone(),
                is_active: enable,
                max_expiry: expiry,
                up: HashMap::new(),
                down: HashMap::new(),
                total_gb: HashMap::new(),
                last_online: HashMap::new(),
            });

            entry.name = email;
            entry.is_active = entry.is_active || enable;
            if expiry > entry.max_expiry {
                entry.max_expiry = expiry;
            }
            entry.up.insert(inbound_id, up);
            entry.down.insert(inbound_id, down);
            entry.total_gb.insert(inbound_id, total_gb);
            entry.last_online.insert(inbound_id, last_online);
        }
    }

    for (uuid, user) in users {
        let (days_left, next_date) = if user.max_expiry > 0 {
            let dt = Utc
                .timestamp_millis_opt(user.max_expiry)
                .single()
                .ok_or_else(|| "invalid expiryTime".to_string())?;
            let diff = dt - now;
            let days = diff.num_days();

            (days, dt.date_naive().to_string())
        } else {
            (0, "".to_string())
        };

        conn.execute(
            "INSERT INTO user_auth (uuid, name, days_left, up, down, total_GB, next_payment_date, is_active, last_online)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(uuid) DO UPDATE SET
                name=excluded.name,
                days_left=excluded.days_left,
                up=excluded.up,
                down=excluded.down,
                total_GB=excluded.total_GB,
                next_payment_date=excluded.next_payment_date,
                is_active=excluded.is_active,
                last_online=excluded.last_online",
            params![
                uuid,
                user.name,
                days_left,
                metric_i64_to_json(&user.up),
                metric_i64_to_json(&user.down),
                metric_f64_to_json(&user.total_gb),
                next_date,
                user.is_active,
                metric_i64_to_json(&user.last_online)
            ]
        ).map_err(|e| e.to_string())?;
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