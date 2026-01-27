//! J.A.R.V.I.S. - Just A Rather Very Intelligent System
//! A voice-activated AI assistant written in Rust

use anyhow::{Context, Result};
use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const VOICE: &str = "Daniel";
const TARGET_SAMPLE_RATE: u32 = 16000;  // What speech recognition APIs expect
const SILENCE_THRESHOLD: f32 = 0.01;
const SILENCE_DURATION: Duration = Duration::from_millis(800);
const MAX_RECORD_DURATION: Duration = Duration::from_secs(6);

// ============== SPEECH OUTPUT ==============

fn speak(text: &str) {
    println!("JARVIS: {}", text);
    let _ = Command::new("say")
        .args(["-v", VOICE, text])
        .status();
}

fn speak_async(text: &str) {
    println!("JARVIS: {}", text);
    let text = text.to_string();
    std::thread::spawn(move || {
        let _ = Command::new("say")
            .args(["-v", VOICE, &text])
            .status();
    });
}

// ============== TIME-BASED GREETING ==============

fn get_greeting() -> &'static str {
    let hour = Local::now().hour();
    match hour {
        5..=11 => "Good morning, sir.",
        12..=16 => "Good afternoon, sir.",
        17..=20 => "Good evening, sir.",
        _ => "Hello, sir. Burning the midnight oil, I see.",
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
    speak_async("Welcome home, sir.");
    
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
        // Try Spotify
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

fn control_music(command: &str) {
    if command.contains("play") {
        let _ = run_applescript(r#"tell application "Music" to play"#);
        speak("Playing music, sir.");
    } else if command.contains("pause") || command.contains("stop") {
        let _ = run_applescript(r#"tell application "Music" to pause"#);
        speak("Music paused, sir.");
    } else if command.contains("next") || command.contains("skip") {
        let _ = run_applescript(r#"tell application "Music" to next track"#);
        speak("Next track, sir.");
    } else if command.contains("previous") || command.contains("back") {
        let _ = run_applescript(r#"tell application "Music" to previous track"#);
        speak("Previous track, sir.");
    } else if command.contains("volume up") || command.contains("louder") {
        let _ = run_applescript(r#"set volume output volume ((output volume of (get volume settings)) + 15)"#);
        speak("Volume increased.");
    } else if command.contains("volume down") || command.contains("quieter") {
        let _ = run_applescript(r#"set volume output volume ((output volume of (get volume settings)) - 15)"#);
        speak("Volume decreased.");
    } else if command.contains("mute") {
        let _ = run_applescript(r#"set volume output muted true"#);
        speak("Muted, sir.");
    } else if command.contains("unmute") {
        let _ = run_applescript(r#"set volume output muted false"#);
        speak("Unmuted, sir.");
    }
}

// ============== APP CONTROL ==============

fn open_app(app_name: &str) {
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
    
    speak(&format!("Opening {}, sir.", actual_app));
    let _ = Command::new("open")
        .args(["-a", actual_app])
        .status();
}

fn close_app(app_name: &str) {
    let script = format!(r#"tell application "{}" to quit"#, app_name);
    let _ = run_applescript(&script);
    speak(&format!("{} closed, sir.", app_name));
}

// ============== SYSTEM INFO ==============

fn get_time() {
    let time = Local::now().format("%I:%M %p");
    speak(&format!("The time is {}, sir.", time));
}

fn get_date() {
    let date = Local::now().format("%A, %B %d, %Y");
    speak(&format!("Today is {}, sir.", date));
}

fn get_battery() {
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
                "charging"
            } else {
                "on battery"
            };
            speak(&format!("Battery is at {} percent, {}, sir.", percent, status));
            return;
        }
    }
    speak("I couldn't determine the battery level, sir.");
}

fn get_weather() {
    let _ = Command::new("open").args(["-a", "Weather"]).status();
    speak("Opening weather, sir.");
}

// ============== WEB & SEARCH ==============

fn search_web(query: &str) {
    speak(&format!("Searching for {}, sir.", query));
    let url = format!("https://www.google.com/search?q={}", urlencoding::encode(query));
    let _ = Command::new("open").arg(&url).status();
}

fn open_website(url: &str) {
    let url = if url.starts_with("http") {
        url.to_string()
    } else {
        format!("https://{}", url)
    };
    speak("Opening website, sir.");
    let _ = Command::new("open").arg(&url).status();
}

// ============== REMINDERS & NOTES ==============

fn set_reminder(text: &str) {
    let script = format!(
        r#"tell application "Reminders" to make new reminder with properties {{name:"{}"}}"#,
        text.replace('"', "\\\"")
    );
    let _ = run_applescript(&script);
    speak(&format!("Reminder set: {}, sir.", text));
}

fn create_note(text: &str) {
    let script = format!(
        r#"tell application "Notes" to make new note with properties {{body:"{}"}}"#,
        text.replace('"', "\\\"")
    );
    let _ = run_applescript(&script);
    speak("Note created, sir.");
}

// ============== COMMUNICATION ==============

fn make_call(contact: &str) {
    let _ = Command::new("open")
        .arg(format!("facetime://{}", contact))
        .status();
    speak(&format!("Calling {}, sir.", contact));
}

// ============== SCREEN & DISPLAY ==============

fn take_screenshot() {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let path = format!(
        "{}/Desktop/screenshot_{}.png",
        std::env::var("HOME").unwrap_or_default(),
        timestamp
    );
    let _ = Command::new("screencapture").args(["-i", &path]).status();
    speak("Screenshot saved to desktop, sir.");
}

fn toggle_dark_mode() {
    let script = r#"
    tell application "System Events"
        tell appearance preferences
            set dark mode to not dark mode
        end tell
    end tell
    "#;
    let _ = run_applescript(script);
    speak("Dark mode toggled, sir.");
}

// ============== SYSTEM CONTROL ==============

fn lock_screen() {
    speak("Locking now, sir.");
    let _ = Command::new("pmset").arg("displaysleepnow").status();
}

fn sleep_computer() {
    speak("Goodnight, sir.");
    let _ = Command::new("pmset").arg("sleepnow").status();
}

fn empty_trash() {
    let _ = run_applescript(r#"tell application "Finder" to empty trash"#);
    speak("Trash emptied, sir.");
}

// ============== AUDIO RECORDING ==============

/// Resample audio from source_rate to target_rate using linear interpolation
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

/// Convert stereo to mono by averaging channels
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

    // Get the device's supported configuration instead of hardcoding
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

            // Check for sound
            let max_amplitude = data.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            if max_amplitude > SILENCE_THRESHOLD {
                *last_sound = Instant::now();
            }

            // Convert f32 to i16 and store
            for &sample in data {
                let s = (sample * 32767.0) as i16;
                samples.push(s);
            }
        },
        |err| eprintln!("Audio error: {}", err),
        None,
    )?;

    stream.play()?;

    // Wait for speech to end (silence detection) or timeout
    loop {
        std::thread::sleep(Duration::from_millis(100));

        let elapsed = start_time.elapsed();
        let silence_elapsed = last_sound.lock().unwrap().elapsed();

        // Stop if max duration reached
        if elapsed > MAX_RECORD_DURATION {
            break;
        }

        // Stop if silence detected after some initial audio
        let samples_len = samples.lock().unwrap().len();
        if samples_len > device_sample_rate as usize && silence_elapsed > SILENCE_DURATION {
            break;
        }
    }

    *recording.lock().unwrap() = false;
    drop(stream);

    let raw_samples = samples.lock().unwrap().clone();

    // Convert stereo to mono if needed
    let mono_samples = stereo_to_mono(&raw_samples, channels);

    // Resample to target rate (16000 Hz) for speech recognition
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
    
    // Use Google's Web Speech API (same as Python's speech_recognition)
    let client = reqwest::blocking::Client::new();
    
    // First request to get the upstream URL
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
    
    // Parse the JSON response (second line contains the result)
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

fn process_command(text: &str) -> bool {
    let text = text.to_lowercase();
    let text = text.trim();
    println!("Processing: {}", text);
    
    // Extract command after wake word
    let command = if text.contains("jarvis") {
        text.split("jarvis").last().unwrap_or("").trim()
    } else {
        text
    };
    
    // === DADDY'S HOME ===
    if text.contains("daddy's home") || text.contains("daddys home") 
        || text.contains("daddy is home") || text.contains("wake up daddy") {
        play_back_in_black();
        return true;
    }
    
    // === GREETING ONLY ===
    if command.is_empty() || command == "hey" || command == "hello" || command == "hi" {
        speak(get_greeting());
        return true;
    }
    
    // === TIME & DATE ===
    if command.contains("time") && (command.contains("what") || command.contains("tell")) {
        get_time();
        return true;
    }
    
    if command.contains("date") || command.contains("day") {
        get_date();
        return true;
    }
    
    // === BATTERY ===
    if command.contains("battery") {
        get_battery();
        return true;
    }
    
    // === WEATHER ===
    if command.contains("weather") {
        get_weather();
        return true;
    }
    
    // === APP CONTROL ===
    if command.starts_with("open ") || command.starts_with("launch ") {
        let app = command
            .trim_start_matches("open ")
            .trim_start_matches("launch ")
            .trim();
        open_app(app);
        return true;
    }
    
    if command.starts_with("close ") || command.starts_with("quit ") {
        let app = command
            .trim_start_matches("close ")
            .trim_start_matches("quit ")
            .trim();
        close_app(app);
        return true;
    }
    
    // === MUSIC ===
    let music_keywords = ["play music", "pause music", "stop music", "next song", 
                          "next track", "skip", "previous", "volume up", "volume down",
                          "louder", "quieter", "mute", "unmute"];
    if music_keywords.iter().any(|k| command.contains(k)) {
        control_music(command);
        return true;
    }
    
    // === WEB SEARCH ===
    if command.starts_with("search ") || command.starts_with("google ") || command.starts_with("look up ") {
        let query = command
            .trim_start_matches("search ")
            .trim_start_matches("search for ")
            .trim_start_matches("google ")
            .trim_start_matches("look up ")
            .trim();
        search_web(query);
        return true;
    }
    
    // === WEBSITES ===
    if command.starts_with("go to ") || command.starts_with("visit ") {
        let url = command
            .trim_start_matches("go to ")
            .trim_start_matches("visit ")
            .trim();
        open_website(url);
        return true;
    }
    
    // === REMINDERS ===
    if command.contains("remind me") || command.starts_with("set reminder") {
        let reminder = command
            .trim_start_matches("remind me to ")
            .trim_start_matches("remind me ")
            .trim_start_matches("set reminder ")
            .trim();
        set_reminder(reminder);
        return true;
    }
    
    // === NOTES ===
    if command.contains("note") {
        let note = command
            .trim_start_matches("take note ")
            .trim_start_matches("create note ")
            .trim_start_matches("note that ")
            .trim();
        create_note(note);
        return true;
    }
    
    // === CALLS ===
    if command.starts_with("call ") {
        let contact = command.trim_start_matches("call ").trim();
        make_call(contact);
        return true;
    }
    
    // === SCREENSHOT ===
    if command.contains("screenshot") || command.contains("screen shot") {
        take_screenshot();
        return true;
    }
    
    // === DARK MODE ===
    if command.contains("dark mode") {
        toggle_dark_mode();
        return true;
    }
    
    // === SYSTEM COMMANDS ===
    if command.contains("lock") && (command.contains("screen") || command.contains("computer")) {
        lock_screen();
        return true;
    }
    
    if command.contains("sleep") || command.contains("goodnight") {
        sleep_computer();
        return true;
    }
    
    if command.contains("empty trash") {
        empty_trash();
        return true;
    }
    
    // === GRATITUDE ===
    if command.contains("thank") {
        speak("At your service, sir.");
        return true;
    }
    
    // === HELP ===
    if command.contains("help") || command.contains("what can you do") {
        speak("I can open and close apps, control music, search the web, tell you the time, check your battery, take screenshots, toggle dark mode, set reminders, and much more. Just ask, sir.");
        return true;
    }
    
    // === STATUS ===
    if command.contains("how are you") || command.contains("status") {
        speak("All systems operational, sir. How may I assist?");
        return true;
    }
    
    // === IDENTITY ===
    if command.contains("who are you") || command.contains("your name") {
        speak("I am Jarvis, sir. Just A Rather Very Intelligent System, at your service.");
        return true;
    }
    
    // === GOODBYE ===
    if command.contains("goodbye") || command.contains("bye") || command.contains("that's all") {
        speak("Very good, sir. I'll be here if you need me.");
        return true;
    }
    
    // === DEFAULT: Search questions ===
    let question_words = ["what is", "what are", "who is", "where is", "when", "why", "how"];
    if question_words.iter().any(|q| command.contains(q)) {
        search_web(command);
        return true;
    }
    
    false
}

// ============== MAIN ==============

fn main() -> Result<()> {
    println!("{}", "=".repeat(60));
    println!("     J.A.R.V.I.S. INITIALIZED");
    println!("     Just A Rather Very Intelligent System");
    println!("     [Rust Edition]");
    println!("{}", "=".repeat(60));
    println!("Wake words: 'Hey Jarvis' | 'Jarvis' | 'Daddy's home'");
    println!("{}", "=".repeat(60));
    
    speak("Jarvis online and at your service, sir.");
    
    loop {
        // Record audio
        match record_audio() {
            Ok(samples) => {
                if samples.len() < 1000 {
                    continue; // Too short, probably noise
                }
                
                // Recognize speech
                match recognize_speech(&samples) {
                    Ok(text) => {
                        if text.is_empty() {
                            continue;
                        }
                        println!("Heard: {}", text);
                        
                        // Check for wake word
                        if text.contains("jarvis") || text.contains("daddy") {
                            if !process_command(&text) {
                                speak("I didn't quite understand that, sir. Could you rephrase?");
                            }
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
