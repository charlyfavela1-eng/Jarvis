//! J.A.R.V.I.S. - Just A Rather Very Intelligent System
//! A voice-activated AI assistant written in Rust
//! Powered by GPT-4o with maximum intelligence settings
//! Features: Memory, Calendar, Email, Real-time Data, Interrupt Capability

use anyhow::{Context, Result};
use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
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

// API Keys for real-time data
const OPENWEATHER_API_KEY: &str = "fde45bd61b21f91fbef3390c50880328";
const NEWS_API_KEY: &str = "1d0a89008a144bc98a07ec4b4f010ea5";
const COINGECKO_API_KEY: &str = "CG-rb5DLsUawaMHYXohQQS3qsua";
const ALPHA_VANTAGE_API_KEY: &str = "550LOGP4K22TKV0Y";

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

// ============== PERSISTENT MEMORY ==============

#[derive(Debug, Serialize, Deserialize, Default)]
struct JarvisMemory {
    preferences: HashMap<String, String>,
    facts: HashMap<String, String>,
    last_updated: String,
}

impl JarvisMemory {
    fn memory_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".jarvis")
    }

    fn memory_path() -> PathBuf {
        Self::memory_dir().join("memory.json")
    }

    fn load() -> Self {
        let path = Self::memory_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(memory) = serde_json::from_str(&content) {
                    return memory;
                }
            }
        }
        Self::default()
    }

    fn save(&self) {
        let dir = Self::memory_dir();
        let _ = fs::create_dir_all(&dir);
        let path = Self::memory_path();
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }

    fn remember(&mut self, key: &str, value: &str) {
        self.preferences.insert(key.to_lowercase(), value.to_string());
        self.last_updated = Local::now().to_rfc3339();
        self.save();
    }

    fn recall(&self, key: &str) -> Option<&String> {
        self.preferences.get(&key.to_lowercase())
    }

    fn add_fact(&mut self, topic: &str, fact: &str) {
        self.facts.insert(topic.to_lowercase(), fact.to_string());
        self.last_updated = Local::now().to_rfc3339();
        self.save();
    }

    fn get_fact(&self, topic: &str) -> Option<&String> {
        self.facts.get(&topic.to_lowercase())
    }
}

// ============== CONVERSATION HISTORY (PERSISTENT) ==============

#[derive(Serialize, Deserialize)]
struct ConversationHistory {
    messages: VecDeque<(String, String)>,
}

impl ConversationHistory {
    fn history_path() -> PathBuf {
        JarvisMemory::memory_dir().join("conversation_history.json")
    }

    fn load_or_new() -> Self {
        let path = Self::history_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(history) = serde_json::from_str(&content) {
                    return history;
                }
            }
        }
        Self {
            messages: VecDeque::new(),
        }
    }

    fn save(&self) {
        let dir = JarvisMemory::memory_dir();
        let _ = fs::create_dir_all(&dir);
        let path = Self::history_path();
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }

    fn add_user(&mut self, content: &str) {
        self.messages
            .push_back(("user".to_string(), content.to_string()));
        self.trim();
        self.save();
    }

    fn add_assistant(&mut self, content: &str) {
        self.messages
            .push_back(("assistant".to_string(), content.to_string()));
        self.trim();
        self.save();
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

// ============== INTERRUPT FLAG ==============

lazy_static::lazy_static! {
    static ref INTERRUPT_REQUESTED: AtomicBool = AtomicBool::new(false);
}

fn request_interrupt() {
    INTERRUPT_REQUESTED.store(true, Ordering::SeqCst);
    kill_existing_speech();
}

fn clear_interrupt() {
    INTERRUPT_REQUESTED.store(false, Ordering::SeqCst);
}

fn is_interrupt_requested() -> bool {
    INTERRUPT_REQUESTED.load(Ordering::SeqCst)
}

// ============== SPEECH OUTPUT (WITH INTERRUPT SUPPORT) ==============

/// Kill any existing speech processes to prevent overlap
fn kill_existing_speech() {
    let _ = Command::new("killall").arg("afplay").output();
}

/// Speak text with interrupt capability - can be stopped by saying "Jarvis" or "stop"
fn speak(text: &str) {
    // Acquire lock to prevent concurrent speech
    let _lock = SPEECH_MUTEX.lock().unwrap();

    // Clear any pending interrupt and kill lingering speech
    clear_interrupt();
    kill_existing_speech();

    IS_SPEAKING.store(true, Ordering::SeqCst);
    println!("\x1b[36mJARVIS:\x1b[0m {}", text);

    // Use ElevenLabs for speech
    match elevenlabs_tts(text) {
        Ok(audio_path) => {
            // Spawn afplay as a child process so we can kill it on interrupt
            if let Ok(mut child) = Command::new("afplay")
                .arg(&audio_path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                // Poll for completion or interrupt
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) => break, // Finished normally
                        Ok(None) => {
                            if is_interrupt_requested() {
                                let _ = child.kill();
                                println!("\x1b[33m[Interrupted]\x1b[0m");
                                clear_interrupt();
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(50));
                        }
                        Err(_) => break,
                    }
                }
            }
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

// ============== CALENDAR INTEGRATION ==============

fn get_todays_events() -> String {
    let script = r#"
    tell application "Calendar"
        set todayStart to (current date)
        set time of todayStart to 0
        set todayEnd to todayStart + (1 * days)

        set eventList to {}
        repeat with cal in calendars
            try
                set theEvents to (every event of cal whose start date >= todayStart and start date < todayEnd)
                repeat with e in theEvents
                    set end of eventList to (summary of e & " at " & time string of (start date of e))
                end repeat
            end try
        end repeat

        if (count of eventList) = 0 then
            return "no events"
        else
            set AppleScript's text item delimiters to ", "
            return eventList as text
        end if
    end tell
    "#;

    match run_applescript(script) {
        Ok(result) => {
            let trimmed = result.trim();
            if trimmed == "no events" || trimmed.is_empty() {
                "Your calendar is clear today, sir. A rarity, if I may say so.".to_string()
            } else {
                format!("Today's agenda includes: {}", trimmed)
            }
        }
        Err(_) => "I'm having trouble accessing your calendar at the moment, sir.".to_string()
    }
}

fn create_calendar_event(title: &str, time_str: &str) -> String {
    // Simple event creation for "tomorrow at X"
    let script = format!(r#"
    tell application "Calendar"
        tell calendar "Calendar"
            set eventDate to (current date) + (1 * days)
            set hours of eventDate to {}
            set minutes of eventDate to 0
            set endDate to eventDate + (1 * hours)
            make new event with properties {{summary:"{}", start date:eventDate, end date:endDate}}
        end tell
    end tell
    return "success"
    "#, time_str, title.replace('"', "\\\""));

    match run_applescript(&script) {
        Ok(_) => format!("I've added {} to your calendar for tomorrow, sir.", title),
        Err(_) => "I couldn't create that calendar event, sir. Please check Calendar permissions.".to_string()
    }
}

// ============== EMAIL INTEGRATION ==============

fn get_unread_emails() -> String {
    let script = r#"
    tell application "Mail"
        set unreadMessages to (messages of inbox whose read status is false)
        set msgCount to count of unreadMessages

        if msgCount = 0 then
            return "no unread emails"
        end if

        set summaryList to {}
        set maxShow to 3
        if msgCount < maxShow then set maxShow to msgCount

        repeat with i from 1 to maxShow
            set msg to item i of unreadMessages
            set msgFrom to sender of msg
            set msgSubject to subject of msg
            set end of summaryList to (msgFrom & " regarding " & msgSubject)
        end repeat

        set AppleScript's text item delimiters to ". "
        return (msgCount as text) & " unread: " & (summaryList as text)
    end tell
    "#;

    match run_applescript(script) {
        Ok(result) => {
            let trimmed = result.trim();
            if trimmed == "no unread emails" {
                "Your inbox is clear, sir. Well done.".to_string()
            } else {
                format!("You have {}", trimmed)
            }
        }
        Err(_) => "I'm having trouble accessing your email at the moment, sir.".to_string()
    }
}

// ============== REAL-TIME DATA APIs ==============

fn get_weather(location: &str) -> String {
    let location = if location.is_empty() { "Plano,TX" } else { location };

    let url = format!(
        "https://api.openweathermap.org/data/2.5/weather?q={}&units=imperial&appid={}",
        urlencoding::encode(location),
        OPENWEATHER_API_KEY
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build();

    if let Ok(client) = client {
        if let Ok(response) = client.get(&url).send() {
            if let Ok(json) = response.json::<serde_json::Value>() {
                let temp = json["main"]["temp"].as_f64().unwrap_or(0.0);
                let feels_like = json["main"]["feels_like"].as_f64().unwrap_or(0.0);
                let description = json["weather"][0]["description"].as_str().unwrap_or("unclear conditions");
                let humidity = json["main"]["humidity"].as_i64().unwrap_or(0);

                return format!(
                    "Currently {} degrees in {}, feels like {} degrees, with {}. Humidity is at {} percent.",
                    temp.round() as i32, location, feels_like.round() as i32, description, humidity
                );
            }
        }
    }

    "I'm having trouble fetching the weather data at the moment, sir.".to_string()
}

fn get_stock_price(symbol: &str) -> String {
    // Using Alpha Vantage API
    let url = format!(
        "https://www.alphavantage.co/query?function=GLOBAL_QUOTE&symbol={}&apikey={}",
        symbol.to_uppercase(),
        ALPHA_VANTAGE_API_KEY
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build();

    if let Ok(client) = client {
        if let Ok(response) = client.get(&url).send() {
            if let Ok(json) = response.json::<serde_json::Value>() {
                if let Some(quote) = json.get("Global Quote") {
                    let price = quote["05. price"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                    let change_pct = quote["10. change percent"].as_str().unwrap_or("0%")
                        .trim_end_matches('%').parse::<f64>().unwrap_or(0.0);
                    let direction = if change_pct >= 0.0 { "up" } else { "down" };

                    if price > 0.0 {
                        return format!(
                            "{} is trading at {:.2} dollars, {} {:.2} percent for the day.",
                            symbol.to_uppercase(), price, direction, change_pct.abs()
                        );
                    }
                }
            }
        }
    }

    format!("I couldn't fetch the stock price for {} at the moment, sir.", symbol)
}

fn get_crypto_price(coin: &str) -> String {
    let coin_id = match coin.to_lowercase().as_str() {
        "bitcoin" | "btc" => "bitcoin",
        "ethereum" | "eth" => "ethereum",
        "dogecoin" | "doge" => "dogecoin",
        "solana" | "sol" => "solana",
        "cardano" | "ada" => "cardano",
        "xrp" | "ripple" => "ripple",
        _ => coin,
    };

    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true&x_cg_demo_api_key={}",
        coin_id,
        COINGECKO_API_KEY
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build();

    if let Ok(client) = client {
        if let Ok(response) = client.get(&url).send() {
            if let Ok(json) = response.json::<serde_json::Value>() {
                if let Some(data) = json.get(coin_id) {
                    let price = data["usd"].as_f64().unwrap_or(0.0);
                    let change = data["usd_24h_change"].as_f64().unwrap_or(0.0);
                    let direction = if change >= 0.0 { "up" } else { "down" };

                    return format!(
                        "{} is at {:.2} dollars, {} {:.1} percent in the last twenty-four hours.",
                        coin, price, direction, change.abs()
                    );
                }
            }
        }
    }

    format!("I couldn't fetch the price for {} at the moment, sir.", coin)
}

fn get_sports_scores(sport: &str) -> String {
    let sport_path = match sport.to_lowercase().as_str() {
        "nfl" | "football" => "football/nfl",
        "nba" | "basketball" => "basketball/nba",
        "mlb" | "baseball" => "baseball/mlb",
        "nhl" | "hockey" => "hockey/nhl",
        "soccer" | "mls" => "soccer/usa.1",
        _ => return format!("I don't have data for {} at the moment, sir.", sport),
    };

    let url = format!(
        "https://site.api.espn.com/apis/site/v2/sports/{}/scoreboard",
        sport_path
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build();

    if let Ok(client) = client {
        if let Ok(response) = client.get(&url).send() {
            if let Ok(json) = response.json::<serde_json::Value>() {
                if let Some(events) = json["events"].as_array() {
                    if events.is_empty() {
                        return format!("No {} games scheduled today, sir.", sport);
                    }

                    let mut scores = Vec::new();
                    for event in events.iter().take(3) {
                        if let Some(name) = event["name"].as_str() {
                            scores.push(name.to_string());
                        }
                    }

                    if scores.is_empty() {
                        return format!("No {} games scheduled today, sir.", sport);
                    }

                    return format!("Today's {} matchups: {}", sport.to_uppercase(), scores.join(", "));
                }
            }
        }
    }

    format!("I couldn't fetch {} scores at the moment, sir.", sport)
}

fn get_news_headlines() -> String {
    let url = format!(
        "https://newsapi.org/v2/top-headlines?country=us&pageSize=5&apiKey={}",
        NEWS_API_KEY
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build();

    if let Ok(client) = client {
        if let Ok(response) = client.get(&url).send() {
            if let Ok(json) = response.json::<serde_json::Value>() {
                if let Some(articles) = json["articles"].as_array() {
                    let headlines: Vec<String> = articles
                        .iter()
                        .take(3)
                        .filter_map(|a| a["title"].as_str().map(|s| s.to_string()))
                        .collect();

                    if !headlines.is_empty() {
                        return format!("Top headlines: {}", headlines.join(". "));
                    }
                }
            }
        }
    }

    "I couldn't fetch the news at the moment, sir.".to_string()
}

// ============== PROACTIVE FEATURES ==============

lazy_static::lazy_static! {
    static ref LATE_NIGHT_SUGGESTED: AtomicBool = AtomicBool::new(false);
}

fn generate_morning_briefing() -> String {
    let mut briefing_parts = Vec::new();

    // Weather
    let weather = get_weather("");
    if !weather.contains("trouble") {
        briefing_parts.push(weather);
    }

    // Calendar
    let calendar = get_todays_events();
    briefing_parts.push(calendar);

    // News (shortened)
    let news = get_news_headlines();
    if !news.contains("couldn't") {
        briefing_parts.push(news);
    }

    format!("Good morning, sir. Here's your briefing: {}", briefing_parts.join(" "))
}

fn check_late_night() -> Option<String> {
    let hour = Local::now().hour();

    // Between 11pm and 4am
    if (hour >= 23 || hour < 4) && !LATE_NIGHT_SUGGESTED.swap(true, Ordering::SeqCst) {
        return Some(
            "Sir, I couldn't help but notice the hour. Perhaps it would be prudent to consider retiring for the evening? Your productivity tomorrow will thank you.".to_string()
        );
    }

    None
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

fn try_execute_action(command: &str, memory: &mut JarvisMemory) -> Option<String> {
    let command = command.to_lowercase();

    // === INTERRUPT / STOP ===
    if command == "stop" || command == "never mind" || command == "cancel" {
        request_interrupt();
        return Some("Of course, sir.".to_string());
    }

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

    // === GOOD MORNING (BRIEFING) ===
    if command.contains("good morning") || command.contains("morning briefing") || command.contains("brief me") {
        return Some(generate_morning_briefing());
    }

    // === MEMORY: REMEMBER ===
    if command.starts_with("remember ") {
        // Parse "remember I like my coffee black" or "remember my favorite color is blue"
        let text = command.trim_start_matches("remember ");

        // Try to extract key-value from "I like my X Y" pattern
        if text.contains(" like my ") || text.contains(" like ") {
            let parts: Vec<&str> = text.split(" like ").collect();
            if parts.len() >= 2 {
                let value_part = parts[1].trim_start_matches("my ");
                let words: Vec<&str> = value_part.split_whitespace().collect();
                if words.len() >= 2 {
                    let key = words[0];
                    let value = words[1..].join(" ");
                    memory.remember(key, &value);
                    return Some(format!("I've made a note that you like your {} {}, sir.", key, value));
                }
            }
        }

        // Try "my X is Y" pattern
        if text.contains(" is ") {
            let parts: Vec<&str> = text.split(" is ").collect();
            if parts.len() >= 2 {
                let key = parts[0].trim_start_matches("my ").trim_start_matches("that my ");
                let value = parts[1].trim();
                memory.remember(key, value);
                return Some(format!("Noted, sir. Your {} is {}.", key, value));
            }
        }

        // Generic remember
        memory.add_fact("note", text);
        return Some("I've made a note of that, sir.".to_string());
    }

    // === MEMORY: RECALL ===
    if command.contains("how do i like") || command.contains("what's my") || command.contains("whats my") || command.contains("what is my") {
        let topic = command
            .replace("how do i like my ", "")
            .replace("how do i like ", "")
            .replace("what's my ", "")
            .replace("whats my ", "")
            .replace("what is my ", "")
            .trim()
            .trim_end_matches('?')
            .to_string();

        if let Some(value) = memory.recall(&topic) {
            return Some(format!("You prefer your {} {}, sir.", topic, value));
        } else if let Some(value) = memory.get_fact(&topic) {
            return Some(format!("According to my records, your {} is {}, sir.", topic, value));
        } else {
            return Some(format!("I don't believe you've told me your {} preference yet, sir.", topic));
        }
    }

    // === CALENDAR ===
    if command.contains("calendar") || command.contains("schedule today") || command.contains("my events") || command.contains("what's on my") {
        return Some(get_todays_events());
    }

    if command.contains("schedule ") && (command.contains("tomorrow") || command.contains("meeting")) {
        // Parse "schedule meeting with John tomorrow at 3"
        let text = command
            .replace("schedule ", "")
            .replace("tomorrow", "")
            .replace(" at ", " ")
            .trim()
            .to_string();

        // Extract hour
        let hour = if command.contains(" at 3") || command.contains(" at three") { "15" }
        else if command.contains(" at 2") || command.contains(" at two") { "14" }
        else if command.contains(" at 4") || command.contains(" at four") { "16" }
        else if command.contains(" at 5") || command.contains(" at five") { "17" }
        else if command.contains(" at 9") || command.contains(" at nine") { "9" }
        else if command.contains(" at 10") || command.contains(" at ten") { "10" }
        else if command.contains(" at 11") || command.contains(" at eleven") { "11" }
        else if command.contains(" at 12") || command.contains(" at noon") || command.contains(" at twelve") { "12" }
        else { "12" };

        let title = text.split_whitespace().take(5).collect::<Vec<&str>>().join(" ");
        return Some(create_calendar_event(&title, hour));
    }

    // === EMAIL ===
    if command.contains("email") && (command.contains("check") || command.contains("unread") || command.contains("any") || command.contains("my")) {
        return Some(get_unread_emails());
    }

    // === WEATHER ===
    if command.contains("weather") {
        let location = command
            .replace("what's the weather", "")
            .replace("whats the weather", "")
            .replace("weather in", "")
            .replace("weather for", "")
            .replace("weather", "")
            .trim()
            .to_string();
        return Some(get_weather(&location));
    }

    // === STOCKS ===
    if command.contains("stock") || command.contains("price of") && !command.contains("crypto") {
        let symbols = ["apple", "tesla", "google", "amazon", "microsoft", "nvidia", "meta"];
        let symbol_map = [
            ("apple", "AAPL"), ("tesla", "TSLA"), ("google", "GOOGL"), ("alphabet", "GOOGL"),
            ("amazon", "AMZN"), ("microsoft", "MSFT"), ("nvidia", "NVDA"), ("meta", "META"),
            ("facebook", "META"), ("netflix", "NFLX"), ("disney", "DIS"),
        ];

        for (name, ticker) in symbol_map.iter() {
            if command.contains(name) {
                return Some(get_stock_price(ticker));
            }
        }

        // Try to extract ticker directly
        let words: Vec<&str> = command.split_whitespace().collect();
        for word in words {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
            if clean.len() >= 1 && clean.len() <= 5 && clean.chars().all(|c| c.is_alphabetic()) {
                if clean.to_uppercase() != "THE" && clean.to_uppercase() != "OF" && clean.to_uppercase() != "FOR" {
                    return Some(get_stock_price(clean));
                }
            }
        }
    }

    // === CRYPTO ===
    if command.contains("bitcoin") || command.contains("btc") {
        return Some(get_crypto_price("bitcoin"));
    }
    if command.contains("ethereum") || command.contains("eth") {
        return Some(get_crypto_price("ethereum"));
    }
    if command.contains("crypto") || command.contains("dogecoin") || command.contains("doge") {
        if command.contains("doge") {
            return Some(get_crypto_price("dogecoin"));
        }
        return Some(get_crypto_price("bitcoin")); // Default to bitcoin
    }

    // === SPORTS ===
    if command.contains("score") || command.contains("game") {
        if command.contains("nfl") || command.contains("football") {
            return Some(get_sports_scores("nfl"));
        }
        if command.contains("nba") || command.contains("basketball") {
            return Some(get_sports_scores("nba"));
        }
        if command.contains("mlb") || command.contains("baseball") {
            return Some(get_sports_scores("mlb"));
        }
        if command.contains("nhl") || command.contains("hockey") {
            return Some(get_sports_scores("nhl"));
        }
        // Default to NFL
        return Some(get_sports_scores("nfl"));
    }

    // === NEWS ===
    if command.contains("news") || command.contains("headlines") {
        return Some(get_news_headlines());
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

fn process_command(text: &str, history: &mut ConversationHistory, memory: &mut JarvisMemory) -> String {
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
    if let Some(response) = try_execute_action(command, memory) {
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
    println!("\x1b[36m     J.A.R.V.I.S. ONLINE\x1b[0m");
    println!("     Just A Rather Very Intelligent System");
    println!("     \x1b[90m[Stark Industries | GPT-4o Enhanced | Memory Enabled]\x1b[0m");
    println!("\x1b[36m{}\x1b[0m", "═".repeat(60));
    println!("\x1b[90mWake words: 'Jarvis' | 'Hey Jarvis' | 'Daddy's home'\x1b[0m");
    println!("\x1b[90mNew: Stocks, Crypto, Weather, Calendar, Email, Memory\x1b[0m");
    println!("\x1b[90mSay 'Jarvis stop' to interrupt\x1b[0m");
    println!("\x1b[36m{}\x1b[0m", "═".repeat(60));

    // Check for OpenAI API key
    if env::var("OPENAI_API_KEY").is_err() {
        eprintln!("\x1b[31mWARNING: OPENAI_API_KEY not set. Advanced AI features will be limited.\x1b[0m");
    }

    // Load persistent memory and conversation history
    let mut memory = JarvisMemory::load();
    let mut history = ConversationHistory::load_or_new();

    println!("\x1b[90mMemory loaded from ~/.jarvis/\x1b[0m");

    // Use local greeting to avoid rate limits on startup
    let greeting = get_greeting();
    speak(&greeting);

    loop {
        // Check for late night proactive suggestion
        if let Some(suggestion) = check_late_night() {
            speak(&suggestion);
        }

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

                        // Check for interrupt while speaking
                        if is_speaking() && (text.contains("jarvis") || text.contains("stop")) {
                            request_interrupt();
                            continue;
                        }

                        // Check for wake word
                        if text.contains("jarvis") || text.contains("daddy") {
                            let response = process_command(&text, &mut history, &mut memory);
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
