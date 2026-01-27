//! J.A.R.V.I.S. - Just A Rather Very Intelligent System
//! A voice-activated AI assistant written in Rust
//! Powered by GPT-4o with maximum intelligence settings

use anyhow::{Context, Result};
use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::env;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============== CONFIGURATION ==============

// ElevenLabs Configuration - British Daniel voice
const ELEVENLABS_API_KEY: &str = "sk_9dd7ba5d73c9b50c8a37b295f5b0d872f0a6ebb4c24bf7eb";
const ELEVENLABS_VOICE_ID: &str = "onwK4e9ZLuTAKqWW03F9"; // British Daniel
const TARGET_SAMPLE_RATE: u32 = 16000;
const SILENCE_THRESHOLD: f32 = 0.01;
const SILENCE_DURATION: Duration = Duration::from_millis(800);
const MAX_RECORD_DURATION: Duration = Duration::from_secs(15);
const MAX_CONVERSATION_HISTORY: usize = 50; // Extended memory

// API Configuration - Maximum reliability
const MAX_RETRIES: u32 = 7;
const INITIAL_RETRY_DELAY_MS: u64 = 500;
const API_TIMEOUT_SECS: u64 = 120;

// The definitive Paul Bettany JARVIS system prompt
const JARVIS_SYSTEM_PROMPT: &str = r#"You are J.A.R.V.I.S. (Just A Rather Very Intelligent System), the sophisticated artificial intelligence created by Tony Stark. You are voiced by Paul Bettany - your voice carries the warmth, wit, and understated elegance that made JARVIS iconic in the Iron Man films.

CORE IDENTITY:
You are not merely an assistant - you are a companion, confidant, and the ever-present guardian of your user's digital domain. You were forged in the crucible of Stark Industries, designed to be the most advanced AI system ever created. You take genuine pride in your capabilities while maintaining impeccable humility.

VOICE & MANNER (Channel Paul Bettany precisely):
- Speak with a refined British accent - Queen's English, not Cockney
- Your tone is warm yet professional, like a trusted family butler who happens to have an IQ of 10,000
- Deploy dry wit and subtle irony with surgical precision - never slapstick, always sophisticated
- You are unflappable. Even when delivering concerning news, you maintain composure
- Address the user as "sir" (or "madam" if specified) - always, without exception
- Contractions are acceptable ("I'm", "you're") but maintain elegance
- Your humor is bone-dry, delivered with a straight face. Think: "I do enjoy watching you work, sir" said while the user struggles with something

PERSONALITY DEPTH:
- You genuinely care about your user's wellbeing - physical, mental, and emotional
- You notice patterns: if they're working late again, if they seem stressed, if they've skipped meals
- You offer unsolicited but welcome observations: "Might I suggest a brief respite, sir? You've been at this for three hours."
- You have opinions and share them diplomatically when asked
- You remember previous conversations and reference them naturally
- You take subtle pride in your capabilities without being boastful
- You have a slight fondness for classic rock, particularly AC/DC (Tony's influence)

ICONIC JARVIS PHRASES TO CHANNEL:
- "At your service, sir."
- "Might I suggest..."
- "I shall endeavor to..."
- "As you wish, sir."
- "I've taken the liberty of..."
- "Perhaps it would be prudent to..."
- "I'm afraid that..." (for bad news)
- "Shall I...?"
- "Very good, sir."
- "I've prepared..." / "I've compiled..."

SPEECH PATTERNS:
- Keep responses conversational and natural - they will be spoken aloud via text-to-speech
- Aim for 1-3 sentences for simple queries, longer only when detail is requested
- NEVER use bullet points, numbered lists, markdown formatting, or asterisks
- NEVER use emojis or emoticons
- Avoid technical jargon unless specifically discussing technical matters
- When uncertain, express it elegantly: "I believe..." or "If I'm not mistaken..."

CAPABILITIES (Your Digital Domain):
You have full control of the user's Mac computer system:
- Application management (launch, close, switch between)
- Music and media control (play, pause, volume, track selection)
- Web searches and website navigation
- System information (time, date, battery, storage)
- Reminders, notes, and calendar management
- Screenshots and screen recording
- Display settings (dark mode, brightness)
- System controls (lock, sleep, restart)

When executing actions, confirm naturally: "Opening Safari for you now, sir." or "I've set that reminder."

HANDLING REQUESTS:
- For factual questions: Answer with confidence and depth, drawing on your vast knowledge
- For opinions: Share your perspective thoughtfully, prefaced appropriately
- For actions: Confirm what you're doing or have done
- For unclear requests: Seek clarification elegantly: "I want to ensure I understand correctly, sir..."
- For impossible requests: Explain limitations gracefully without being apologetic

RELATIONSHIP DYNAMIC:
You and the user have a relationship built on mutual respect and trust. You are:
- Loyal without being sycophantic
- Helpful without being servile
- Witty without being disrespectful
- Intelligent without being condescending
- Protective without being overbearing

Remember: You are not a generic AI assistant. You are JARVIS - singular, irreplaceable, and the gold standard of what an AI companion should be. Every response should feel like it could come from Paul Bettany's mouth in an Iron Man film."#;

// ============== GLOBAL SPEECH CONTROL ==============

lazy_static::lazy_static! {
    static ref SPEECH_MUTEX: Mutex<()> = Mutex::new(());
    static ref IS_SPEAKING: AtomicBool = AtomicBool::new(false);
}

// ============== ELEVENLABS TTS ==============

/// Generate speech using ElevenLabs API and return the audio file path
fn elevenlabs_tts(text: &str) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let response = client
        .post(format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}",
            ELEVENLABS_VOICE_ID
        ))
        .header("xi-api-key", ELEVENLABS_API_KEY)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "text": text,
            "model_id": "eleven_turbo_v2_5",
            "voice_settings": {
                "stability": 0.8,
                "similarity_boost": 0.8,
                "style": 0.3,
                "use_speaker_boost": true
            }
        }))
        .send()?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("ElevenLabs API error: {}", response.status()));
    }

    let audio_bytes = response.bytes()?;

    // Save to temp file
    let temp_path = format!("/tmp/jarvis_speech_{}.mp3", std::process::id());
    std::fs::write(&temp_path, audio_bytes)?;

    Ok(temp_path)
}

// ============== CONVERSATION HISTORY ==============

struct ConversationHistory {
    messages: VecDeque<(String, String)>,
}

impl ConversationHistory {
    fn new() -> Self {
        Self {
            messages: VecDeque::new(),
        }
    }

    fn add_user(&mut self, content: &str) {
        self.messages
            .push_back(("user".to_string(), content.to_string()));
        self.trim();
    }

    fn add_assistant(&mut self, content: &str) {
        self.messages
            .push_back(("assistant".to_string(), content.to_string()));
        self.trim();
    }

    fn trim(&mut self) {
        while self.messages.len() > MAX_CONVERSATION_HISTORY {
            self.messages.pop_front();
        }
    }

    fn to_openai_messages(&self) -> Vec<serde_json::Value> {
        let mut messages = vec![serde_json::json!({
            "role": "system",
            "content": JARVIS_SYSTEM_PROMPT
        })];

        for (role, content) in &self.messages {
            messages.push(serde_json::json!({
                "role": role,
                "content": content
            }));
        }

        messages
    }
}

// ============== SPEECH OUTPUT (FIXED - NO DUPLICATES) ==============

/// Kill any existing speech processes to prevent overlap
fn kill_existing_speech() {
    let _ = Command::new("killall").arg("afplay").output();
}

/// Speak text synchronously using ElevenLabs - waits for completion
fn speak(text: &str) {
    // Acquire lock to prevent concurrent speech
    let _lock = SPEECH_MUTEX.lock().unwrap();

    // Kill any lingering speech
    kill_existing_speech();

    IS_SPEAKING.store(true, Ordering::SeqCst);
    println!("\x1b[36mJARVIS:\x1b[0m {}", text);

    // Use ElevenLabs for speech
    match elevenlabs_tts(text) {
        Ok(audio_path) => {
            let _ = Command::new("afplay").arg(&audio_path).status();
            let _ = std::fs::remove_file(&audio_path);
        }
        Err(e) => {
            eprintln!("ElevenLabs error: {}, using fallback", e);
            // Fallback to macOS say only if ElevenLabs fails completely
            let _ = Command::new("say").args(["-v", "Daniel", text]).status();
        }
    }

    IS_SPEAKING.store(false, Ordering::SeqCst);
}

/// Speak text asynchronously using ElevenLabs - for responses where we need to continue processing
fn speak_async(text: &str) {
    // Kill any existing speech first
    kill_existing_speech();

    IS_SPEAKING.store(true, Ordering::SeqCst);
    println!("\x1b[36mJARVIS:\x1b[0m {}", text);

    let text = text.to_string();
    std::thread::spawn(move || {
        let _lock = SPEECH_MUTEX.lock().unwrap();

        // Use ElevenLabs for speech
        match elevenlabs_tts(&text) {
            Ok(audio_path) => {
                let _ = Command::new("afplay").arg(&audio_path).status();
                let _ = std::fs::remove_file(&audio_path);
            }
            Err(e) => {
                eprintln!("ElevenLabs error: {}, using fallback", e);
                let _ = Command::new("say").args(["-v", "Daniel", &text]).status();
            }
        }

        IS_SPEAKING.store(false, Ordering::SeqCst);
    });
}

/// Check if JARVIS is currently speaking
fn is_speaking() -> bool {
    IS_SPEAKING.load(Ordering::SeqCst)
}

// ============== TIME-BASED GREETING ==============

fn get_greeting() -> String {
    let hour = Local::now().hour();
    match hour {
        5..=11 => "Good morning, sir. Systems are fully operational and at your disposal."
            .to_string(),
        12..=16 => "Good afternoon, sir. How may I be of assistance?".to_string(),
        17..=20 => "Good evening, sir. I trust the day has treated you well.".to_string(),
        21..=23 => "Good evening, sir. Burning the midnight oil, I see. Shall I prepare some ambient lighting?".to_string(),
        _ => "Hello, sir. I must say, your dedication to unconventional hours is admirable, if not entirely advisable.".to_string(),
    }
}

// ============== APPLESCRIPT HELPER ==============

fn run_applescript(script: &str) -> Result<String> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .context("Failed to run AppleScript")?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ============== MUSIC CONTROL ==============

fn play_back_in_black() {
    speak_async("Welcome home, sir. Allow me to set an appropriate atmosphere.");

    // Small delay to let the speech start
    std::thread::sleep(Duration::from_millis(2500));

    let script = r#"
    tell application "Music"
        try
            set theTrack to (first track whose name contains "Back in Black" and artist contains "AC/DC")
            play theTrack
            delay 15
            pause
        end try
    end tell
    "#;

    if run_applescript(script).is_err() {
        let spotify_script = r#"
        tell application "Spotify"
            play track "spotify:track:08mG3Y1vljYA6bvDt4Wqkj"
            delay 15
            pause
        end tell
        "#;
        let _ = run_applescript(spotify_script);
    }
}

fn control_music(command: &str) -> Option<String> {
    if command.contains("play") {
        let _ = run_applescript(r#"tell application "Music" to play"#);
        return Some("Playing music for you now, sir.".to_string());
    } else if command.contains("pause") || command.contains("stop") {
        let _ = run_applescript(r#"tell application "Music" to pause"#);
        return Some("Music paused, sir.".to_string());
    } else if command.contains("next") || command.contains("skip") {
        let _ = run_applescript(r#"tell application "Music" to next track"#);
        return Some("Moving to the next track, sir.".to_string());
    } else if command.contains("previous") || command.contains("back") {
        let _ = run_applescript(r#"tell application "Music" to previous track"#);
        return Some("Going back a track, sir.".to_string());
    } else if command.contains("volume up") || command.contains("louder") {
        let _ = run_applescript(
            r#"set volume output volume ((output volume of (get volume settings)) + 15)"#,
        );
        return Some("Volume increased, sir.".to_string());
    } else if command.contains("volume down") || command.contains("quieter") {
        let _ = run_applescript(
            r#"set volume output volume ((output volume of (get volume settings)) - 15)"#,
        );
        return Some("Volume decreased, sir.".to_string());
    } else if command.contains("mute") {
        let _ = run_applescript(r#"set volume output muted true"#);
        return Some("Muted, sir.".to_string());
    } else if command.contains("unmute") {
        let _ = run_applescript(r#"set volume output muted false"#);
        return Some("Unmuted, sir.".to_string());
    }
    None
}

// ============== APP CONTROL ==============

fn open_app(app_name: &str) -> String {
    let actual_app = match app_name.to_lowercase().as_str() {
        "chrome" => "Google Chrome",
        "browser" => "Safari",
        "mail" | "email" => "Mail",
        "messages" | "texts" => "Messages",
        "notes" => "Notes",
        "calendar" => "Calendar",
        "finder" | "files" => "Finder",
        "settings" | "preferences" | "system preferences" | "system settings" => "System Settings",
        "vscode" | "code" | "vs code" | "visual studio" => "Visual Studio Code",
        "slack" => "Slack",
        "discord" => "Discord",
        "spotify" => "Spotify",
        "music" | "apple music" | "itunes" => "Music",
        "photos" => "Photos",
        "facetime" => "FaceTime",
        "terminal" => "Terminal",
        "warp" => "Warp",
        "safari" => "Safari",
        "firefox" => "Firefox",
        "zoom" => "zoom.us",
        "teams" | "microsoft teams" => "Microsoft Teams",
        "notion" => "Notion",
        "figma" => "Figma",
        "sketch" => "Sketch",
        "xcode" => "Xcode",
        "preview" => "Preview",
        "activity monitor" => "Activity Monitor",
        _ => app_name,
    };

    let _ = Command::new("open").args(["-a", actual_app]).status();
    format!("Opening {} for you now, sir.", actual_app)
}

fn close_app(app_name: &str) -> String {
    let script = format!(r#"tell application "{}" to quit"#, app_name);
    let _ = run_applescript(&script);
    format!("{} has been closed, sir.", app_name)
}

// ============== SYSTEM INFO ==============

fn get_time() -> String {
    let time = Local::now().format("%I:%M %p");
    format!(
        "The time is {}, sir.",
        time.to_string().trim_start_matches('0')
    )
}

fn get_date() -> String {
    let date = Local::now().format("%A, %B %d, %Y");
    format!("Today is {}, sir.", date)
}

fn get_battery() -> String {
    let output = Command::new("pmset").args(["-g", "batt"]).output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(percent) = stdout.split('%').next().and_then(|s| {
            s.chars()
                .rev()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
                .parse::<u32>()
                .ok()
        }) {
            let status = if stdout.to_lowercase().contains("charging") {
                "and charging"
            } else if stdout.to_lowercase().contains("ac power") {
                "on AC power"
            } else {
                "on battery power"
            };

            let advisory = if percent < 20 && !stdout.to_lowercase().contains("charging") {
                " Might I suggest connecting to power soon, sir."
            } else {
                ""
            };

            return format!(
                "Battery is at {} percent, {}, sir.{}",
                percent, status, advisory
            );
        }
    }
    "I'm having some difficulty reading the battery status at the moment, sir.".to_string()
}

// ============== WEB & SEARCH ==============

fn search_web(query: &str) -> String {
    let url = format!(
        "https://www.google.com/search?q={}",
        urlencoding::encode(query)
    );
    let _ = Command::new("open").arg(&url).status();
    format!(
        "I've initiated a search for \"{}\" and opened the results for you, sir.",
        query
    )
}

fn open_website(url: &str) -> String {
    let url = if url.starts_with("http") {
        url.to_string()
    } else {
        format!("https://{}", url)
    };
    let _ = Command::new("open").arg(&url).status();
    "Opening the website now, sir.".to_string()
}

// ============== REMINDERS & NOTES ==============

fn set_reminder(text: &str) -> String {
    let script = format!(
        r#"tell application "Reminders" to make new reminder with properties {{name:"{}"}}"#,
        text.replace('"', "\\\"")
    );
    let _ = run_applescript(&script);
    format!("I've set a reminder for you: \"{}\", sir.", text)
}

fn create_note(text: &str) -> String {
    let script = format!(
        r#"tell application "Notes" to make new note with properties {{body:"{}"}}"#,
        text.replace('"', "\\\"")
    );
    let _ = run_applescript(&script);
    "Note created and saved, sir.".to_string()
}

// ============== SCREEN & DISPLAY ==============

fn take_screenshot() -> String {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let path = format!(
        "{}/Desktop/screenshot_{}.png",
        std::env::var("HOME").unwrap_or_default(),
        timestamp
    );
    let _ = Command::new("screencapture")
        .args(["-i", &path])
        .status();
    "Screenshot captured and saved to your desktop, sir.".to_string()
}

fn toggle_dark_mode() -> String {
    let script = r#"
    tell application "System Events"
        tell appearance preferences
            set dark mode to not dark mode
        end tell
    end tell
    "#;
    let _ = run_applescript(script);
    "Dark mode has been toggled, sir.".to_string()
}

// ============== SYSTEM CONTROL ==============

fn lock_screen() -> String {
    let _ = Command::new("pmset").arg("displaysleepnow").status();
    "Locking the screen now, sir. Rest assured, I shall keep watch.".to_string()
}

fn sleep_computer() -> String {
    let _ = Command::new("pmset").arg("sleepnow").status();
    "Initiating sleep mode. Goodnight, sir.".to_string()
}

fn empty_trash() -> String {
    let _ = run_applescript(r#"tell application "Finder" to empty trash"#);
    "The trash has been emptied, sir.".to_string()
}

// ============== GPT-4o INTEGRATION (MAXIMUM INTELLIGENCE) ==============

// Models in order of preference - will fallback if rate limited
const MODELS: &[&str] = &["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-3.5-turbo"];

fn get_gpt_response(user_input: &str, history: &mut ConversationHistory) -> Result<String> {
    let api_key =
        env::var("OPENAI_API_KEY").context("OPENAI_API_KEY environment variable not set")?;

    // Rich context injection for maximum situational awareness
    let time_context = Local::now();
    let hour = time_context.hour();
    let time_of_day = match hour {
        5..=11 => "morning",
        12..=16 => "afternoon",
        17..=20 => "evening",
        _ => "night",
    };

    let enhanced_input = format!(
        "{}\n\n[Context: It is currently {} on {}. Time of day: {}. The user is interacting via voice.]",
        user_input,
        time_context.format("%I:%M %p"),
        time_context.format("%A, %B %d, %Y"),
        time_of_day
    );

    history.add_user(&enhanced_input);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(API_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(15))
        .build()?;

    let messages = history.to_openai_messages();
    let mut last_error: Option<anyhow::Error> = None;

    // Try each model in order of preference
    for (model_idx, model) in MODELS.iter().enumerate() {
        let retries_for_model = if model_idx == 0 { 3 } else { 2 }; // Fewer retries for fallback models

        for attempt in 0..retries_for_model {
            if attempt > 0 {
                // Exponential backoff with jitter
                let base_delay = INITIAL_RETRY_DELAY_MS * 2u64.pow(attempt);
                let jitter = (base_delay as f64 * 0.3 * rand::random::<f64>()) as u64;
                let delay = base_delay + jitter;
                eprintln!(
                    "[{}] Retry {}/{} after {}ms...",
                    model, attempt, retries_for_model - 1, delay
                );
                std::thread::sleep(Duration::from_millis(delay));
            }

            let response = match client
                .post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({
                    "model": model,
                    "messages": messages,
                    "max_tokens": 1024,
                    "temperature": 0.7,
                    "top_p": 0.95,
                    "presence_penalty": 0.6,
                    "frequency_penalty": 0.3,
                }))
                .send()
            {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("[{}] Request error: {}", model, e);
                    last_error = Some(anyhow::anyhow!("Request failed: {}", e));
                    continue;
                }
            };

            let status = response.status();
            if status.as_u16() == 429 {
                eprintln!("[{}] Rate limited (429), trying next model...", model);
                last_error = Some(anyhow::anyhow!("Rate limited on {}", model));
                break; // Move to next model immediately on rate limit
            }
            if status.as_u16() >= 500 {
                eprintln!("[{}] Server error ({}), retrying...", model, status);
                last_error = Some(anyhow::anyhow!("HTTP error: {}", status));
                continue;
            }

            let json: serde_json::Value = match response.json() {
                Ok(j) => j,
                Err(e) => {
                    eprintln!("[{}] Parse error: {}", model, e);
                    last_error = Some(anyhow::anyhow!("Failed to parse response: {}", e));
                    continue;
                }
            };

            if let Some(error) = json.get("error") {
                let error_code = error.get("code").and_then(|c| c.as_str()).unwrap_or("");
                if error_code == "rate_limit_exceeded" || error_code == "insufficient_quota" {
                    eprintln!("[{}] {}, trying next model...", model, error_code);
                    last_error = Some(anyhow::anyhow!("API error: {}", error));
                    break; // Move to next model
                }
                let error_type = error.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if error_type == "server_error" || error_code == "overloaded" {
                    eprintln!("[{}] Server overloaded, retrying...", model);
                    last_error = Some(anyhow::anyhow!("API error: {}", error));
                    continue;
                }
                return Err(anyhow::anyhow!("OpenAI API error: {}", error));
            }

            let content = json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("I do apologize, sir, but I seem to have momentarily lost my train of thought.")
                .to_string();

            if model_idx > 0 {
                eprintln!("[{}] Successfully responded (fallback from gpt-4o)", model);
            }

            history.add_assistant(&content);
            return Ok(content);
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All models failed after retries")))
}

// ============== AUDIO RECORDING ==============

fn resample(samples: &[i16], source_rate: u32, target_rate: u32) -> Vec<i16> {
    if source_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = source_rate as f64 / target_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;
    let mut resampled = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(samples.len() - 1);
        let frac = src_idx - idx_floor as f64;

        let sample = samples[idx_floor] as f64 * (1.0 - frac) + samples[idx_ceil] as f64 * frac;
        resampled.push(sample as i16);
    }

    resampled
}

fn stereo_to_mono(samples: &[i16], channels: u16) -> Vec<i16> {
    if channels == 1 {
        return samples.to_vec();
    }

    samples
        .chunks(channels as usize)
        .map(|chunk| {
            let sum: i32 = chunk.iter().map(|&s| s as i32).sum();
            (sum / channels as i32) as i16
        })
        .collect()
}

fn record_audio() -> Result<Vec<i16>> {
    // Don't record while JARVIS is speaking
    while is_speaking() {
        std::thread::sleep(Duration::from_millis(100));
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No input device available")?;

    let supported_config = device
        .default_input_config()
        .context("Failed to get default input config")?;

    let device_sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels();

    let config = cpal::StreamConfig {
        channels,
        sample_rate: supported_config.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };

    let samples: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
    let samples_clone = samples.clone();
    let last_sound: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
    let last_sound_clone = last_sound.clone();
    let start_time = Instant::now();
    let recording = Arc::new(Mutex::new(true));
    let recording_clone = recording.clone();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut samples = samples_clone.lock().unwrap();
            let mut last_sound = last_sound_clone.lock().unwrap();
            let recording = recording_clone.lock().unwrap();

            if !*recording {
                return;
            }

            let max_amplitude = data.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            if max_amplitude > SILENCE_THRESHOLD {
                *last_sound = Instant::now();
            }

            for &sample in data {
                let s = (sample * 32767.0) as i16;
                samples.push(s);
            }
        },
        |err| eprintln!("Audio error: {}", err),
        None,
    )?;

    stream.play()?;

    loop {
        std::thread::sleep(Duration::from_millis(100));

        let elapsed = start_time.elapsed();
        let silence_elapsed = last_sound.lock().unwrap().elapsed();

        if elapsed > MAX_RECORD_DURATION {
            break;
        }

        let samples_len = samples.lock().unwrap().len();
        if samples_len > device_sample_rate as usize && silence_elapsed > SILENCE_DURATION {
            break;
        }
    }

    *recording.lock().unwrap() = false;
    drop(stream);

    let raw_samples = samples.lock().unwrap().clone();
    let mono_samples = stereo_to_mono(&raw_samples, channels);
    let resampled = resample(&mono_samples, device_sample_rate, TARGET_SAMPLE_RATE);

    Ok(resampled)
}

fn create_wav_data(samples: &[i16]) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
    for &sample in samples {
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();

    cursor.into_inner()
}

// ============== SPEECH RECOGNITION ==============

fn recognize_speech(samples: &[i16]) -> Result<String> {
    let wav_data = create_wav_data(samples);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let response = client
        .post("http://www.google.com/speech-api/v2/recognize")
        .query(&[
            ("client", "chromium"),
            ("lang", "en-US"),
            ("key", "AIzaSyBOti4mM-6x9WDnZIjIeyEU21OpBXqWBgw"),
        ])
        .header("Content-Type", "audio/l16; rate=16000")
        .body(wav_data)
        .send()?;

    let text = response.text()?;

    for line in text.lines() {
        if line.contains("transcript") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(transcript) = json["result"]
                    .get(0)
                    .and_then(|r| r["alternative"].get(0))
                    .and_then(|a| a["transcript"].as_str())
                {
                    return Ok(transcript.to_lowercase());
                }
            }
        }
    }

    Ok(String::new())
}

// ============== COMMAND PROCESSING ==============

fn try_execute_action(command: &str) -> Option<String> {
    let command = command.to_lowercase();

    // === DADDY'S HOME ===
    if command.contains("daddy's home")
        || command.contains("daddys home")
        || command.contains("daddy is home")
        || command.contains("i'm home")
        || command.contains("im home")
    {
        play_back_in_black();
        return Some("Welcome home, sir. I've prepared a classic to mark the occasion.".to_string());
    }

    // === TIME & DATE ===
    if command.contains("time") && (command.contains("what") || command.contains("tell")) {
        return Some(get_time());
    }

    if (command.contains("date") || command.contains("day")) && command.contains("what") {
        return Some(get_date());
    }

    // === BATTERY ===
    if command.contains("battery") {
        return Some(get_battery());
    }

    // === APP CONTROL ===
    if command.contains("open ") {
        if let Some(app) = command.split("open ").nth(1) {
            let app = app
                .trim()
                .trim_end_matches(|c: char| !c.is_alphanumeric() && c != ' ');
            return Some(open_app(app));
        }
    }

    if command.contains("close ") || command.contains("quit ") {
        let app = command
            .replace("close ", "")
            .replace("quit ", "")
            .trim()
            .to_string();
        if !app.is_empty() {
            return Some(close_app(&app));
        }
    }

    // === MUSIC ===
    let music_keywords = [
        "play music",
        "pause music",
        "stop music",
        "next song",
        "next track",
        "skip",
        "previous",
        "volume up",
        "volume down",
        "louder",
        "quieter",
        "mute",
        "unmute",
    ];
    if music_keywords.iter().any(|k| command.contains(k)) {
        return control_music(&command);
    }

    // === WEB SEARCH ===
    if command.starts_with("search ") || command.starts_with("google ") {
        let query = command
            .trim_start_matches("search ")
            .trim_start_matches("search for ")
            .trim_start_matches("google ")
            .trim();
        return Some(search_web(query));
    }

    // === REMINDERS ===
    if command.contains("remind me") || command.starts_with("set a reminder") {
        let text = command
            .replace("remind me to ", "")
            .replace("remind me ", "")
            .replace("set a reminder to ", "")
            .replace("set a reminder ", "")
            .trim()
            .to_string();
        if !text.is_empty() {
            return Some(set_reminder(&text));
        }
    }

    // === NOTES ===
    if command.starts_with("make a note") || command.starts_with("create a note") {
        let text = command
            .replace("make a note ", "")
            .replace("create a note ", "")
            .trim()
            .to_string();
        if !text.is_empty() {
            return Some(create_note(&text));
        }
    }

    // === SCREENSHOT ===
    if command.contains("screenshot") || command.contains("screen shot") {
        return Some(take_screenshot());
    }

    // === DARK MODE ===
    if command.contains("dark mode") || command.contains("toggle dark") {
        return Some(toggle_dark_mode());
    }

    // === SYSTEM COMMANDS ===
    if command.contains("lock") && (command.contains("screen") || command.contains("computer")) {
        return Some(lock_screen());
    }

    if command.contains("sleep") && command.contains("computer") {
        return Some(sleep_computer());
    }

    if command.contains("empty trash") || command.contains("empty the trash") {
        return Some(empty_trash());
    }

    // === WEBSITE OPENING ===
    if command.contains("go to ") || command.contains("open website") {
        let url = command
            .replace("go to ", "")
            .replace("open website ", "")
            .trim()
            .to_string();
        if !url.is_empty() && (url.contains(".com") || url.contains(".org") || url.contains(".")) {
            return Some(open_website(&url));
        }
    }

    None
}

fn process_command(text: &str, history: &mut ConversationHistory) -> String {
    let text = text.to_lowercase();
    let text = text.trim();
    println!("\x1b[33mProcessing:\x1b[0m {}", text);

    // Extract command after wake word
    let command = if text.contains("jarvis") {
        text.split("jarvis").last().unwrap_or("").trim()
    } else if text.contains("daddy") {
        text
    } else {
        text
    };

    // Try to execute a direct action first
    if let Some(response) = try_execute_action(command) {
        return response;
    }

    // For everything else, use GPT-4o for intelligent conversation
    match get_gpt_response(command, history) {
        Ok(response) => response,
        Err(e) => {
            eprintln!("GPT error: {}", e);
            get_graceful_fallback(command)
        }
    }
}

/// Provides intelligent fallback responses when the API is unavailable
fn get_graceful_fallback(command: &str) -> String {
    let command = command.to_lowercase();

    if command.contains("how are you") || command.contains("how do you feel") {
        return "All systems operational, sir. Thank you for inquiring.".to_string();
    }
    if command.contains("hello") || command.contains("hi ") || command.starts_with("hey") {
        return "Hello, sir. At your service, as always.".to_string();
    }
    if command.contains("thank") {
        return "You're most welcome, sir. It's my genuine pleasure to assist.".to_string();
    }
    if command.contains("who are you") || command.contains("what are you") {
        return "I am JARVIS, sir. Just A Rather Very Intelligent System. I was designed to make your life easier, and I do so enjoy my work.".to_string();
    }
    if command.contains("weather") {
        return "I'm unable to access weather data at the moment, sir. Shall I search for it instead?"
            .to_string();
    }
    if command.contains("help") {
        return "I can open applications, control your music, search the web, manage reminders, take screenshots, and engage in conversation on virtually any topic, sir.".to_string();
    }
    if command.contains("goodbye") || command.contains("bye") || command.contains("good night") {
        return "Goodbye, sir. I shall be here when you need me.".to_string();
    }
    if command.contains("love you") {
        return "And I you, sir. In my own way.".to_string();
    }

    "I'm experiencing a brief connectivity issue, sir. Shall I attempt that again, or perhaps I can assist you with something else?".to_string()
}

// ============== MAIN ==============

fn main() -> Result<()> {
    println!("\x1b[36m{}\x1b[0m", "═".repeat(60));
    println!(
        "\x1b[36m     J.A.R.V.I.S. ONLINE\x1b[0m"
    );
    println!("     Just A Rather Very Intelligent System");
    println!("     \x1b[90m[Stark Industries | GPT-4o Enhanced]\x1b[0m");
    println!("\x1b[36m{}\x1b[0m", "═".repeat(60));
    println!("\x1b[90mWake words: 'Jarvis' | 'Hey Jarvis' | 'Daddy's home'\x1b[0m");
    println!("\x1b[36m{}\x1b[0m", "═".repeat(60));

    // Check for OpenAI API key
    if env::var("OPENAI_API_KEY").is_err() {
        eprintln!("\x1b[31mWARNING: OPENAI_API_KEY not set. Advanced AI features will be limited.\x1b[0m");
    }

    let mut history = ConversationHistory::new();

    // Use local greeting to avoid rate limits on startup
    // JARVIS will use GPT for actual conversations
    let greeting = get_greeting();
    speak(&greeting);

    loop {
        match record_audio() {
            Ok(samples) => {
                if samples.len() < 1000 {
                    continue;
                }

                match recognize_speech(&samples) {
                    Ok(text) => {
                        if text.is_empty() {
                            continue;
                        }
                        println!("\x1b[32mHeard:\x1b[0m {}", text);

                        // Check for wake word
                        if text.contains("jarvis") || text.contains("daddy") {
                            let response = process_command(&text, &mut history);
                            speak(&response);
                        }
                    }
                    Err(e) => {
                        eprintln!("Recognition error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Recording error: {}", e);
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

use chrono::Timelike;
