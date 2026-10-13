use eframe::egui::{self, RichText};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use crate::util::job::{Job, take_finished};
use crate::widgets::button::primary;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StagingPrefs {
    pub auth_url: String,
    pub decode_url: String,
    pub email: String,
}

impl Default for StagingPrefs {
    fn default() -> Self {
        Self {
            auth_url: "https://dev-auth-dg.duluin.com/api/auth/customer/signin".into(),
            decode_url: "https://dev-api-dg.duluin.com/api/v1/thirdparty/duluin-wallet/qris/decode".into(),
            email: String::new(),
        }
    }
}

/// Login and decode state. The password and token live only in memory.
#[derive(Default)]
pub struct Staging {
    password: String,
    token: Option<String>,
    job: Option<Job<Outcome>>,
    report: Option<Result<String, String>>,
}

struct Outcome {
    token: Option<String>,
    report: Result<String, String>,
}

pub fn show(ui: &mut egui::Ui, staging: &mut Staging, prefs: &mut StagingPrefs, payload: &str) {
    if let Some(outcome) = take_finished(&mut staging.job) {
        staging.token = outcome.token;
        staging.report = Some(outcome.report);
    }
    ui.weak("Logs in as the employee (this ends their other sessions), then sends the code to the decode endpoint. Fictional merchants decode, but paying them fails.");
    ui.add_space(4.0);
    egui::Grid::new("qris_staging").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Login URL");
        ui.add(egui::TextEdit::singleline(&mut prefs.auth_url).desired_width(f32::INFINITY));
        ui.end_row();
        ui.label("Decode URL");
        ui.add(egui::TextEdit::singleline(&mut prefs.decode_url).desired_width(f32::INFINITY));
        ui.end_row();
        if staging.token.is_none() {
            ui.label("Email");
            ui.add(egui::TextEdit::singleline(&mut prefs.email).hint_text("Employee with a bound wallet"));
            ui.end_row();
            ui.label("Password");
            ui.add(egui::TextEdit::singleline(&mut staging.password).password(true));
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        let running = staging.job.is_some();
        let ready = !payload.trim().is_empty() && (staging.token.is_some() || !prefs.email.is_empty() && !staging.password.is_empty());
        if ui.add_enabled(ready && !running, primary(ui, "Check on staging")).clicked() {
            let (prefs, password, token, payload) = (prefs.clone(), staging.password.clone(), staging.token.clone(), payload.trim().to_string());
            staging.job = Some(Job::spawn(ui.ctx(), move || check(&prefs, &password, token, &payload)));
        }
        if running {
            ui.spinner();
        }
        if staging.token.is_some() && ui.button("Log out").clicked() {
            staging.token = None;
        }
    });
    if let Some(report) = &staging.report {
        let (text, color) = match report {
            Ok(text) => (text.as_str(), ui.visuals().text_color()),
            Err(text) => (text.as_str(), ui.visuals().error_fg_color),
        };
        ui.add(egui::Label::new(RichText::new(text).monospace().color(color)).selectable(true));
    }
}

fn check(prefs: &StagingPrefs, password: &str, token: Option<String>, payload: &str) -> Outcome {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
    let token = match token {
        Some(token) => token,
        None => match login(&agent, prefs, password) {
            Ok(token) => token,
            Err(error) => return Outcome { token: None, report: Err(error) },
        },
    };
    let report = decode(&agent, prefs, &token, payload);
    // A rejected token means the session ended elsewhere; drop it so the next check logs in again.
    let keep = !matches!(&report, Err(e) if e.starts_with("HTTP 401"));
    Outcome { token: keep.then_some(token), report }
}

fn login(agent: &ureq::Agent, prefs: &StagingPrefs, password: &str) -> Result<String, String> {
    let mut response = agent
        .post(&prefs.auth_url)
        .header("Accept", "application/json")
        .send_json(json!({ "email": prefs.email, "password": password }))
        .map_err(|e| format!("Login request failed: {e}"))?;
    let status = response.status().as_u16();
    let body: Value = response.body_mut().read_json().unwrap_or(Value::Null);
    body.get("access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("Login failed (HTTP {status}): {}", message(&body)))
}

fn decode(agent: &ureq::Agent, prefs: &StagingPrefs, token: &str, payload: &str) -> Result<String, String> {
    let mut response = agent
        .post(&prefs.decode_url)
        .header("Accept", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .send_json(json!({ "qr_content": payload }))
        .map_err(|e| format!("Decode request failed: {e}"))?;
    let status = response.status().as_u16();
    let body: Value = response.body_mut().read_json().unwrap_or(Value::Null);
    let mut report = format!("HTTP {status}: {}", message(&body));
    if let Some(data) = body.get("data").filter(|d| !d.is_null()) {
        report += "\n";
        report += &serde_json::to_string_pretty(data).unwrap_or_default();
    }
    if (200..300).contains(&status) { Ok(report) } else { Err(report) }
}

fn message(body: &Value) -> String {
    body.get("message").and_then(Value::as_str).unwrap_or("").to_string()
}
