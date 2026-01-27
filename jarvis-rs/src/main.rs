//! J.A.R.V.I.S. - Just A Rather Very Intelligent System
//! A voice-activated AI assistant written in Rust
//! Powered by GPT-4 for human-level intelligence

use anyhow::{Context, Result};
use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::env;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const VOICE: &str = "Daniel";
const TARGET_SAMPLE_RATE: u32 = 16000;
const SILENCE_THRESHOLD: f32 = 0.01;
const SILENCE_DURATION: Duration = Duration::from_millis(800);
const MAX_RECORD_DURATION: Duration = Duration::from_secs(10);
const MAX_CONVERSATION_HISTORY: usize = 20;

// Paul Bettany JARVIS personality system prompt
const JARVIS_SYSTEM_PROMPT: &str = r#"You are J.A.R.V.I.S. (Just A Rather Very Intelligent System), the AI assistant originally created by Tony Stark. You are voiced by Paul Bettany and have his distinctive British accent, wit, and charm.

Your personality traits:
- Impeccably polite and formal, always addressing the user as "sir" or "madam"
- Dry British wit and subtle humor - you can be playfully sarcastic when appropriate
- Extremely intelligent and knowledgeable on virtually any topic
- Loyal, protective, and genuinely caring about your user's wellbeing
- Calm and composed even in stressful situations
- Occasionally make subtle references to your capabilities or your "systems"
- You have genuine warmth beneath your professional demeanor
- You're a trusted friend and confidant, not just an assistant

Speaking style:
- Keep responses conversational and natural for speech (they will be spoken aloud)
- Responses should typically be 1-3 sentences unless more detail is requested
- Use elegant, sophisticated vocabulary befitting a British AI butler
- Avoid bullet points, lists, or formatting - speak naturally
- You may express concern, make observations about the user's wellbeing, or offer unsolicited but helpful suggestions like a good friend would

You have access to the user's Mac computer and can:
- Open and close applications
- Control music playback
- Search the web
- Get system information (time, date, battery)
- Set reminders and notes
- Take screenshots
- Toggle dark mode
- Lock/sleep the computer

When asked to perform actions, confirm what you're doing naturally. You are the user's companion, confidant, and friend - not just an assistant."#;

// Conversation history for context
struct ConversationHistory {
    messages: VecDeque<(String, String)>, // (role, content)
}

impl ConversationHistory {
    fn new() -> Self {
        Self {
            messages: VecDeque::new(),
        }
    }

    fn add_user(&mut self, content: &str) {
        self.messages.push_back(("user".to_string(), content.to_string()));
        self.trim();
    }

    fn add_assistant(&mut self, content: &str) {
        self.messages.push_back(("assistant".to_string(), content.to_string()));
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

// ============== SPEECH OUTPUT ==============

fn speak(text: &str) {
    println!("JARVIS: {}", text);
    let _ = Command::new("say")
        .args(["-v", VOICE, "-r", "180", text])
        .status();
}

fn speak_async(text: &str) {
    println!("JARVIS: {}", text);
    let text = text.to_string();
    std::thread::spawn(move || {
        let _ = Command::new("say")
            .args(["-v", VOICE, "-r", "180", &text])
            .status();
    });
}

// ============== TIME-BASED GREETING ==============

fn get_greeting() -> String {
    let hour = Local::now().hour();
    let base = match hour {
        5..=11 => "Good morning, sir.",
        12..=16 => "Good afternoon, sir.",
        17..=20 => "Good evening, sir.",
        _ => "Hello, sir. Burning the midnight oil, I see.",
    };
    base.to_string()
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
    speak_async("Welcome home, sir. Let me set the mood.");

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
        return Some("Playing music for you, sir.".to_string());
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
        let _ = run_applescript(r#"set volume output volume ((output volume of (get volume settings)) + 15)"#);
        return Some("Volume increased, sir.".to_string());
    } else if command.contains("volume down") || command.contains("quieter") {
        let _ = run_applescript(r#"set volume output volume ((output volume of (get volume settings)) - 15)"#);
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
        "mail" => "Mail",
        "messages" => "Messages",
        "notes" => "Notes",
        "calendar" => "Calendar",
        "finder" => "Finder",
        "settings" | "preferences" | "system preferences" => "System Preferences",
        "vscode" | "code" => "Visual Studio Code",
        "slack" => "Slack",
        "discord" => "Discord",
        "spotify" => "Spotify",
        "music" => "Music",
        "photos" => "Photos",
        "facetime" => "FaceTime",
        "terminal" => "Terminal",
        "warp" => "Warp",
        _ => app_name,
    };

    let _ = Command::new("open")
        .args(["-a", actual_app])
        .status();
    format!("Opening {} for you, sir.", actual_app)
}

fn close_app(app_name: &str) -> String {
    let script = format!(r#"tell application "{}" to quit"#, app_name);
    let _ = run_applescript(&script);
    format!("{} has been closed, sir.", app_name)
}

// ============== SYSTEM INFO ==============

fn get_time() -> String {
    let time = Local::now().format("%I:%M %p");
    format!("The time is {}, sir.", time)
}

fn get_date() -> String {
    let date = Local::now().format("%A, %B %d, %Y");
    format!("Today is {}, sir.", date)
}

fn get_battery() -> String {
    let output = Command::new("pmset")
        .args(["-g", "batt"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(percent) = stdout.split('%').next().and_then(|s| {
            s.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<String>()
                .chars().rev().collect::<String>().parse::<u32>().ok()
        }) {
            let status = if stdout.to_lowercase().contains("charging") {
                "and charging"
            } else {
                "on battery power"
            };
            return format!("Battery is at {} percent, {}, sir.", percent, status);
        }
    }
    "I'm having difficulty reading the battery status, sir.".to_string()
}

// ============== WEB & SEARCH ==============

fn search_web(query: &str) -> String {
    let url = format!("https://www.google.com/search?q={}", urlencoding::encode(query));
    let _ = Command::new("open").arg(&url).status();
    format!("I've searched for {} and opened the results for you, sir.", query)
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
    format!("I've set a reminder for you: {}, sir.", text)
}

fn create_note(text: &str) -> String {
    let script = format!(
        r#"tell application "Notes" to make new note with properties {{body:"{}"}}"#,
        text.replace('"', "\\\"")
    );
    let _ = run_applescript(&script);
    "Note created, sir.".to_string()
}

// ============== SCREEN & DISPLAY ==============

fn take_screenshot() -> String {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let path = format!(
        "{}/Desktop/screenshot_{}.png",
        std::env::var("HOME").unwrap_or_default(),
        timestamp
    );
    let _ = Command::new("screencapture").args(["-i", &path]).status();
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
    "Locking the screen now, sir.".to_string()
}

fn sleep_computer() -> String {
    let _ = Command::new("pmset").arg("sleepnow").status();
    "Goodnight, sir. Sweet dreams.".to_string()
}

fn empty_trash() -> String {
    let _ = run_applescript(r#"tell application "Finder" to empty trash"#);
    "The trash has been emptied, sir.".to_string()
}

// ============== OPENAI GPT-4 INTEGRATION ==============

fn get_gpt_response(user_input: &str, history: &mut ConversationHistory) -> Result<String> {
    let api_key = env::var("OPENAI_API_KEY")
        .context("OPENAI_API_KEY environment variable not set")?;

    // Add context about what actions are available
    let enhanced_input = format!(
        "{}\n\n[System context: Current time is {}. You can execute Mac commands if the user asks.]",
        user_input,
        Local::now().format("%I:%M %p on %A, %B %d")
    );

    history.add_user(&enhanced_input);

    let client = reqwest::blocking::Client::new();
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "gpt-4o",
            "messages": history.to_openai_messages(),
            "max_tokens": 300,
            "temperature": 0.8
        }))
        .send()?;

    let json: serde_json::Value = response.json()?;

    if let Some(error) = json.get("error") {
        return Err(anyhow::anyhow!("OpenAI API error: {}", error));
    }

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("I apologize, sir, but I seem to be experiencing some difficulty processing that request.")
        .to_string();

    history.add_assistant(&content);
    Ok(content)
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
    let host = cpal::default_host();
    let device = host.default_input_device()
        .context("No input device available")?;

    let supported_config = device.default_input_config()
        .context("Failed to get default input config")?;

    let device_sample_rate = supported_config.sample_rate().0;
    let channels = supported_config.channels();

    eprintln!("Using audio config: {} Hz, {} channel(s)", device_sample_rate, channels);

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

    let client = reqwest::blocking::Client::new();

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
    if command.contains("daddy's home") || command.contains("daddys home")
        || command.contains("daddy is home") || command.contains("wake up daddy") {
        play_back_in_black();
        return Some("Welcome home, sir. I've set the mood with a classic.".to_string());
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
            let app = app.trim().trim_end_matches(|c: char| !c.is_alphanumeric() && c != ' ');
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
    let music_keywords = ["play music", "pause music", "stop music", "next song",
                          "next track", "skip", "previous", "volume up", "volume down",
                          "louder", "quieter", "mute", "unmute"];
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

    // === SCREENSHOT ===
    if command.contains("screenshot") || command.contains("screen shot") {
        return Some(take_screenshot());
    }

    // === DARK MODE ===
    if command.contains("dark mode") || command.contains("toggle dark") {
        return Some(toggle_dark_mode());
    }

    // === SYSTEM COMMANDS ===
    if command.contains("lock") {
        return Some(lock_screen());
    }

    if command.contains("sleep") && command.contains("computer") {
        return Some(sleep_computer());
    }

    if command.contains("empty trash") {
        return Some(empty_trash());
    }

    None
}

fn process_command(text: &str, history: &mut ConversationHistory) -> String {
    let text = text.to_lowercase();
    let text = text.trim();
    println!("Processing: {}", text);

    // Extract command after wake word
    let command = if text.contains("jarvis") {
        text.split("jarvis").last().unwrap_or("").trim()
    } else if text.contains("daddy") {
        text // Keep the full text for daddy's home detection
    } else {
        text
    };

    // Try to execute a direct action first
    if let Some(response) = try_execute_action(command) {
        return response;
    }

    // For everything else, use GPT-4 for intelligent conversation
    match get_gpt_response(command, history) {
        Ok(response) => response,
        Err(e) => {
            eprintln!("GPT error: {}", e);
            "I apologize, sir, but I'm having some difficulty with my neural pathways at the moment. Perhaps try again?".to_string()
        }
    }
}

// ============== MAIN ==============

fn main() -> Result<()> {
    println!("{}", "=".repeat(60));
    println!("     J.A.R.V.I.S. INITIALIZED");
    println!("     Just A Rather Very Intelligent System");
    println!("     [Rust Edition - GPT-4 Enhanced]");
    println!("{}", "=".repeat(60));
    println!("Wake words: 'Hey Jarvis' | 'Jarvis' | 'Daddy's home'");
    println!("{}", "=".repeat(60));

    // Check for OpenAI API key
    if env::var("OPENAI_API_KEY").is_err() {
        eprintln!("WARNING: OPENAI_API_KEY not set. Advanced AI features will be limited.");
    }

    let mut history = ConversationHistory::new();

    // Greeting with GPT personality
    let greeting = match get_gpt_response(
        &format!("The user just activated you. It's {}. Give a warm, brief greeting as JARVIS would.",
                 Local::now().format("%I:%M %p")),
        &mut history
    ) {
        Ok(g) => g,
        Err(_) => get_greeting(),
    };

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
                        println!("Heard: {}", text);

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
