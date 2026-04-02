
pub fn color_fmt_ok(s: &str, vars: &[&str]) -> String {
    let t = "\x1b[1m\x1b[32m";
    let v = "\x1b[2m";
    let r = "\x1b[0m";

    let mut out = String::new();
    let mut parts = s.split("{}");

    for (i, part) in parts.by_ref().enumerate() {
        out.push_str(t);
        out.push_str(part);
        out.push_str(r);
        if i < vars.len() {
            out.push_str(v);
            out.push_str(vars[i]);
            out.push_str(r);
        }
    }
    out.push_str(r);
    out.to_string()
}
pub fn color_fmt_err(s: &str, vars: &[&str]) -> String {
    let t = "\x1b[1m\x1b[31m";
    let v = "\x1b[2m";
    let r = "\x1b[0m";

    let mut out = String::new();
    let mut parts = s.split("{}");

    for (i, part) in parts.by_ref().enumerate() {
        out.push_str(t);
        out.push_str(part);
        out.push_str(r);
        if i < vars.len() {
            out.push_str(v);
            out.push_str(vars[i]);
            out.push_str(r);
        }
    }
    out.push_str(r);
    out.to_string()
}
pub fn color_fmt_log(s: &str, vars: &[&str]) -> String {
    let t = "\x1b[1m";
    let v = "\x1b[2m";
    let r = "\x1b[0m";

    let mut out = String::new();
    let mut parts = s.split("{}");

    for (i, part) in parts.by_ref().enumerate() {
        out.push_str(t);
        out.push_str(part);
        out.push_str(r);
        if i < vars.len() {
            out.push_str(v);
            out.push_str(vars[i]);
            out.push_str(r);
        }
    }
    out.push_str(r);
    out.to_string()
}