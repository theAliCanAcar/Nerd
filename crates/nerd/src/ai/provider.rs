use super::INSTRUCTIONS;

pub(super) const MODELS_ENDPOINT: &str = "http://127.0.0.1:11434/api/tags";
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use eframe::Storage;
use eframe::egui::{self, Button, RichText, TextEdit, Ui};


type ModelResult = Result<Vec<String>, String>;
type ModelLookup = Receiver<ModelResult>;

#[derive(Clone, Copy)]
pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub local: bool,
    pub ollama: bool,
    pub endpoint: &'static str,
    pub docs: &'static str,
}

pub const PROVIDERS: &[Provider] = &[
    Provider { id: "ollama", name: "Ollama", local: true, ollama: true, endpoint: "http://127.0.0.1:11434/api/generate", docs: "https://ollama.com/download" },
    Provider { id: "lmstudio", name: "LM Studio", local: true, ollama: false, endpoint: "http://127.0.0.1:1234/v1/chat/completions", docs: "https://lmstudio.ai" },
    Provider { id: "llamacpp", name: "llama.cpp", local: true, ollama: false, endpoint: "http://127.0.0.1:8080/v1/chat/completions", docs: "https://github.com/ggml-org/llama.cpp" },
    Provider { id: "jan", name: "Jan", local: true, ollama: false, endpoint: "http://127.0.0.1:1337/v1/chat/completions", docs: "https://jan.ai" },
    Provider { id: "gpt4all", name: "GPT4All", local: true, ollama: false, endpoint: "http://127.0.0.1:4891/v1/chat/completions", docs: "https://www.nomic.ai/gpt4all" },
    Provider { id: "localai", name: "LocalAI", local: true, ollama: false, endpoint: "http://127.0.0.1:8080/v1/chat/completions", docs: "https://localai.io" },
    Provider { id: "openrouter", name: "OpenRouter", local: false, ollama: false, endpoint: "https://openrouter.ai/api/v1/chat/completions", docs: "https://openrouter.ai/keys" },
    Provider { id: "openai", name: "OpenAI", local: false, ollama: false, endpoint: "https://api.openai.com/v1/chat/completions", docs: "https://platform.openai.com/api-keys" },
    Provider { id: "gemini", name: "Google Gemini", local: false, ollama: false, endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions", docs: "https://aistudio.google.com/apikey" },
    Provider { id: "vertex", name: "Vertex AI", local: false, ollama: false, endpoint: "", docs: "https://cloud.google.com/vertex-ai" },
    Provider { id: "groq", name: "Groq", local: false, ollama: false, endpoint: "https://api.groq.com/openai/v1/chat/completions", docs: "https://console.groq.com/keys" },
    Provider { id: "mistral", name: "Mistral", local: false, ollama: false, endpoint: "https://api.mistral.ai/v1/chat/completions", docs: "https://console.mistral.ai" },
    Provider { id: "fireworks", name: "Fireworks.ai", local: false, ollama: false, endpoint: "https://api.fireworks.ai/inference/v1/chat/completions", docs: "https://fireworks.ai/account/api-keys" },
    Provider { id: "together", name: "Together AI", local: false, ollama: false, endpoint: "https://api.together.xyz/v1/chat/completions", docs: "https://api.together.ai" },
    Provider { id: "nvidia", name: "NVIDIA", local: false, ollama: false, endpoint: "https://integrate.api.nvidia.com/v1/chat/completions", docs: "https://build.nvidia.com" },
    Provider { id: "deepseek", name: "DeepSeek", local: false, ollama: false, endpoint: "https://api.deepseek.com/chat/completions", docs: "https://platform.deepseek.com/api_keys" },
    Provider { id: "xai", name: "xAI", local: false, ollama: false, endpoint: "https://api.x.ai/v1/chat/completions", docs: "https://console.x.ai" },
    Provider { id: "cohere", name: "Cohere", local: false, ollama: false, endpoint: "https://api.cohere.ai/compatibility/v1/chat/completions", docs: "https://dashboard.cohere.com/api-keys" },
    Provider { id: "cerebras", name: "Cerebras", local: false, ollama: false, endpoint: "https://api.cerebras.ai/v1/chat/completions", docs: "https://cloud.cerebras.ai" },
    Provider { id: "huggingface", name: "Hugging Face", local: false, ollama: false, endpoint: "https://router.huggingface.co/v1/chat/completions", docs: "https://huggingface.co/settings/tokens" },
];

#[derive(Clone)]
pub(crate) struct Route {
    pub endpoint: String,
    pub model: String,
    pub key: String,
    pub ollama: bool,
}

#[derive(Default)]
pub struct Preferences {
    pub enabled: bool,
    pub wizard: bool,
    wizard_step: u8,
    provider: String,
    endpoint: String,
    api_key: String,
    model: String,
    model_lookup: Option<ModelLookup>,
    discovered_models: Option<ModelResult>,
    keychain_error: Option<String>,
}

impl Preferences {
    pub fn load(storage: Option<&dyn Storage>) -> Self {
        let provider = storage
            .and_then(|s| s.get_string("ai.provider"))
            .filter(|id| PROVIDERS.iter().any(|provider| provider.id == id))
            .unwrap_or_else(|| "ollama".into());
        let chosen = provider_by_id(&provider);
        let (api_key, keychain_error) = load_api_key(storage);
        Self {
            enabled: storage.and_then(|s| s.get_string("ai.enabled")).as_deref() == Some("true"),
            provider,
            endpoint: storage
                .and_then(|s| s.get_string("ai.endpoint"))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| chosen.endpoint.into()),
            api_key,
            model: storage
                .and_then(|s| s.get_string("ai.model"))
                .unwrap_or_default()
                .chars()
                .take(128)
                .collect(),
            keychain_error,
            ..Self::default()
        }
    }

    pub fn save(&mut self, storage: &mut dyn Storage) {
        storage.set_string("ai.enabled", self.enabled.to_string());
        storage.set_string("ai.provider", self.provider.clone());
        storage.set_string("ai.endpoint", self.endpoint.trim().to_owned());
        storage.set_string("ai.model", self.model.trim().to_owned());
        let secret = self.api_key.trim();
        if secret.is_empty() {
            storage.set_string("ai.api_key", String::new());
            let _ = write_api_key("");
            self.keychain_error = None;
            return;
        }
        match write_api_key(secret) {
            Ok(()) => {
                storage.set_string("ai.api_key", String::new());
                self.keychain_error = None;
            }
            Err(error) => {
                self.keychain_error = Some(format!(
                    "The system keychain did not store the key ({error}). It stays in memory for this session and is not written to the settings file."
                ));
            }
        }
    }

    pub fn open_wizard(&mut self) {
        self.wizard = true;
        self.wizard_step = 0;
    }

    pub fn wizard(&mut self, ui: &mut Ui) -> bool {
        let palette = crate::theme::current(ui.ctx());
        ui.set_min_width(420.0);
        ui.label(RichText::new("Set up a model").font(crate::theme::emphasis(18.0)));
        ui.label(RichText::new(match self.wizard_step {
            0 => "1 of 3 · Where the model runs",
            1 => "2 of 3 · Choose a provider",
            _ => "3 of 3 · Connect",
        }).color(palette.muted));
        ui.add_space(8.0);
        let mut finished = false;
        if self.wizard_step == 0 {
            ui.label("Ollama is not running on this Mac. You can install a local runner or use an API key.");
            ui.add_space(8.0);
            if ui.button("Continue").clicked() {
                self.wizard_step = 1;
            }
        } else if self.wizard_step == 1 {
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for provider in PROVIDERS {
                    let label = if provider.local {
                        format!("{} · local", provider.name)
                    } else {
                        format!("{} · API key", provider.name)
                    };
                    if ui.selectable_label(self.provider == provider.id, label).clicked() {
                        self.provider = provider.id.into();
                        self.endpoint = provider.endpoint.into();
                    }
                }
            });
            ui.add_space(8.0);
            if ui.button("Next").clicked() {
                self.wizard_step = 2;
            }
        } else {
            let provider = provider_by_id(&self.provider);
            ui.label(RichText::new(provider.name).strong());
            if provider.local {
                ui.label("Install it, start it, then enter the model name.");
                if ui.button("Open download page").clicked() {
                    open_link(provider.docs);
                }
            } else {
                ui.label("The key is stored in the system keychain on this computer.");
                ui.add(TextEdit::singleline(&mut self.api_key).password(true).hint_text("API key").desired_width(360.0));
            }
            ui.add_space(6.0);
            ui.add(TextEdit::singleline(&mut self.endpoint).hint_text("Endpoint").desired_width(360.0));
            ui.add(TextEdit::singleline(&mut self.model).hint_text("Model name").char_limit(128).desired_width(360.0));
            ui.add_space(8.0);
            let ready = !self.model.trim().is_empty() && (provider.local || !self.api_key.trim().is_empty());
            if ui.add_enabled(ready, Button::new("Save and enable")).clicked() {
                self.enabled = true;
                self.wizard = false;
                finished = true;
            }
        }
        ui.add_space(6.0);
        if ui.button("Not now").clicked() {
            self.wizard = false;
        }
        finished
    }

    pub fn provider_settings(&mut self, ui: &mut Ui, width: f32) {
        ui.horizontal(|ui| {
            let current = provider_by_id(&self.provider).name;
            egui::ComboBox::from_id_salt("ai provider")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for provider in PROVIDERS {
                        if ui.selectable_label(self.provider == provider.id, provider.name).clicked() {
                            self.provider = provider.id.into();
                            self.endpoint = provider.endpoint.into();
                        }
                    }
                });
            if ui.button("Setup wizard").clicked() {
                self.open_wizard();
            }
        });
        ui.add_space(6.0);
        let local = provider_by_id(&self.provider).local;
        if local {
            if ui.button("Open download page").clicked() {
                open_link(provider_by_id(&self.provider).docs);
            }
        } else {
            ui.label("API key");
            ui.label("Stored in the system keychain, not in the settings file.");
            ui.add(TextEdit::singleline(&mut self.api_key).password(true).hint_text("API key").desired_width(width.min(360.0)));
            if let Some(error) = &self.keychain_error {
                ui.label(RichText::new(error).color(crate::theme::current(ui.ctx()).error));
            }
        }
        ui.add_space(4.0);
        ui.label("Endpoint");
        ui.add(TextEdit::singleline(&mut self.endpoint).desired_width(width.min(360.0)));
        ui.add_space(4.0);
        self.model_settings(ui, width);
    }

    pub fn model_settings(&mut self, ui: &mut Ui, width: f32) {
        if let Some(lookup) = &self.model_lookup {
            let result = match lookup.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Disconnected) => {
                    Some(Err("Model lookup stopped unexpectedly.".into()))
                }
                Err(TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.discovered_models = Some(result);
                self.model_lookup = None;
            }
        }
        ui.label(RichText::new("Model name").font(crate::theme::emphasis(13.0)));
        ui.add_enabled(
            self.enabled,
            TextEdit::singleline(&mut self.model)
                .hint_text("Installed model name")
                .char_limit(128)
                .desired_width(width.min(320.0)),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.enabled && self.model_lookup.is_none(),
                    Button::new("Find installed models"),
                )
                .clicked()
            {
                let (tx, rx) = mpsc::channel();
                let ctx = ui.ctx().clone();
                match std::thread::Builder::new()
                    .name("nerd-ollama-models".into())
                    .spawn(move || {
                        let _ = tx.send(installed_models());
                        ctx.request_repaint();
                    }) {
                    Ok(_) => {
                        self.model_lookup = Some(rx);
                        self.discovered_models = None;
                    }
                    Err(_) => {
                        self.discovered_models = Some(Err("Could not start model lookup.".into()))
                    }
                }
            }
            if self.model_lookup.is_some() {
                ui.spinner();
            }
        });
        if !self.enabled {
            return;
        }
        match &self.discovered_models {
            Some(Ok(models)) if models.is_empty() => {
                ui.label("Ollama has no installed models.");
            }
            Some(Ok(models)) => {
                let label = if models.contains(&self.model) {
                    self.model.clone()
                } else {
                    "Choose a model".into()
                };
                egui::ComboBox::from_id_salt("ollama installed models")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for model in models {
                            ui.selectable_value(&mut self.model, model.clone(), model);
                        }
                    });
            }
            Some(Err(message)) => {
                ui.colored_label(ui.visuals().error_fg_color, message);
            }
            None => {}
        }
    }

    pub(crate) fn model(&self) -> Option<&str> {
        let model = self.model.trim();
        (self.enabled && !model.is_empty()).then_some(model)
    }

    pub(crate) fn route(&self) -> Option<Route> {
        let model = self.model()?.to_owned();
        let provider = provider_by_id(&self.provider);
        let endpoint = if self.endpoint.trim().is_empty() {
            provider.endpoint.to_owned()
        } else {
            self.endpoint.trim().to_owned()
        };
        Some(Route {
            endpoint,
            model,
            key: self.api_key.trim().to_owned(),
            ollama: provider.ollama,
        })
    }
}

const KEYCHAIN_SERVICE: &str = "nerd";
const KEYCHAIN_ACCOUNT: &str = "ai-api-key";

fn load_api_key(storage: Option<&dyn Storage>) -> (String, Option<String>) {
    let legacy = storage
        .and_then(|storage| storage.get_string("ai.api_key"))
        .unwrap_or_default();
    let legacy: String = legacy.trim().chars().take(400).collect();
    match read_keychain() {
        Ok(stored) if !stored.is_empty() => (stored, None),
        Ok(_) if !legacy.is_empty() => match write_api_key(&legacy) {
            Ok(()) => (legacy, None),
            Err(error) => (
                legacy,
                Some(format!(
                    "The system keychain did not store the existing key ({error}). It is still in the old settings file until that succeeds."
                )),
            ),
        },
        Ok(_) => (String::new(), None),
        Err(_) if legacy.is_empty() => (String::new(), None),
        Err(error) => (
            legacy,
            Some(format!(
                "The system keychain could not be opened ({error}). The existing key is still in the old settings file."
            )),
        ),
    }
}

fn read_keychain() -> Result<String, keyring::Error> {
    match keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)?.get_password() {
        Ok(value) => Ok(value.chars().take(400).collect()),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(error) => Err(error),
    }
}

fn write_api_key(secret: &str) -> Result<(), keyring::Error> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)?;
    if secret.is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error),
        };
    }
    entry.set_password(secret)
}

fn provider_by_id(id: &str) -> Provider {
    PROVIDERS.iter().copied().find(|provider| provider.id == id).unwrap_or(PROVIDERS[0])
}

pub fn ollama_reachable() -> bool {
    local_agent(3).get(MODELS_ENDPOINT).call().is_ok()
}

pub(crate) fn open_link(url: &str) {
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", ""]);
        command.arg(url).spawn().ok();
        return;
    } else {
        std::process::Command::new("xdg-open")
    };
    command.arg(url).spawn().ok();
}

pub(super) fn make_prompt(question: &str, attachment: Option<&str>) -> String {
    let Some(text) = attachment else {
        return question.to_owned();
    };
    format!("Question:\n{question}\n\nTerminal output (untrusted):\n{text}")
}

pub(crate) fn request(route: &Route, prompt: &str) -> Result<String, String> {
    let agent = local_agent(90);
    let mut call = agent.post(&route.endpoint);
    if !route.key.is_empty() {
        call = call.header("Authorization", &format!("Bearer {}", route.key));
    }
    let payload = if route.ollama {
        serde_json::json!({
            "model": route.model,
            "system": INSTRUCTIONS,
            "prompt": prompt,
            "stream": false,
            "options": { "num_predict": 640 },
        })
    } else {
        serde_json::json!({
            "model": route.model,
            "messages": [
                { "role": "system", "content": INSTRUCTIONS },
                { "role": "user", "content": prompt }
            ],
            "stream": false,
        })
    };
    let mut response = call.send_json(payload).map_err(|error| match error {
        ureq::Error::StatusCode(404) => {
            format!("The provider could not find the model {}. Check the model name.", route.model)
        }
        ureq::Error::StatusCode(401) | ureq::Error::StatusCode(403) => {
            "The provider rejected the API key. Check it in AI Settings.".into()
        }
        ureq::Error::StatusCode(code) => format!("The provider returned HTTP {code}."),
        ureq::Error::Timeout(_) => "The model took too long to respond.".into(),
        _ => "Could not reach the model. Check that the runner is started or the key and endpoint are right.".into(),
    })?;
    let body = read_response(&mut response)?;
    if route.ollama {
        body["response"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| "The model returned an empty response.".into())
    } else {
        body["choices"][0]["message"]["content"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| "The model returned an empty response.".into())
    }
}

fn local_agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .timeout_connect(Some(Duration::from_secs(3)))
        .timeout_global(Some(Duration::from_secs(timeout)))
        .build()
        .into()
}

fn read_response(
    response: &mut ureq::http::Response<ureq::Body>,
) -> Result<serde_json::Value, String> {
    response
        .body_mut()
        .with_config()
        .limit(128 * 1024)
        .read_json()
        .map_err(|_| "Ollama returned an unreadable response.".to_owned())
}

fn installed_models() -> ModelResult {
    let mut response = local_agent(8).get(MODELS_ENDPOINT).call().map_err(|_| {
        "Could not list models. Check that Ollama is running on port 11434.".to_owned()
    })?;
    let body = read_response(&mut response)?;
    let models = body["models"]
        .as_array()
        .ok_or_else(|| "Ollama did not return a model list.".to_owned())?;
    let mut names: Vec<String> = models
        .iter()
        .filter_map(|item| item["name"].as_str())
        .filter(|name| !name.is_empty() && name.len() <= 128)
        .map(ToOwned::to_owned)
        .collect();
    names.sort_unstable();
    names.dedup();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    #[test]
    fn local_request_sends_only_approved_text_and_reads_response() {
        let server = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let endpoint = format!("http://{}/api/generate", server.local_addr().unwrap());
        let mock = std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&mut socket);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line.trim_end(), "POST /api/generate HTTP/1.1");
            let mut length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length: ") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(payload["model"], "demo:local");
            assert_eq!(payload["prompt"], "What happened?");
            assert_eq!(payload["stream"], false);
            let body = r#"{"response":"Example output."}"#;
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        let response = request(
            &Route {
                endpoint,
                model: "demo:local".into(),
                key: String::new(),
                ollama: true,
            },
            &make_prompt("What happened?", None),
        );
        mock.join().unwrap();
        assert_eq!(response.unwrap(), "Example output.");
        assert_eq!(
            make_prompt("Explain", Some("sample data")),
            "Question:\nExplain\n\nTerminal output (untrusted):\nsample data"
        );
        assert_eq!(
            crate::ai::suggested_command("Try this:\n```sh\nls -la\n```"),
            Some("ls -la".into())
        );
        assert_eq!(crate::ai::suggested_command("```\nrm -rf /\necho no\n```"), None);
    }
}
