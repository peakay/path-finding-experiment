use web_time::{Duration, Instant};

/// Manages animation state for UI elements
pub struct AnimationController {
    animation_frame: u64,
    last_animation_update: Instant,
}

impl AnimationController {
    pub fn new() -> Self {
        Self {
            animation_frame: 0,
            last_animation_update: Instant::now(),
        }
    }

    /// Update animation frame based on elapsed time
    pub fn update(&mut self) {
        let now = Instant::now();
        // Update animation frame every 150ms
        if now.duration_since(self.last_animation_update) >= Duration::from_millis(150) {
            self.animation_frame = self.animation_frame.wrapping_add(1);
            self.last_animation_update = now;
        }
    }

    /// Generate animated "Thinking..." text with elaborate effects
    pub fn get_thinking_text(&self) -> String {
        let frame = self.animation_frame;
        let cycle = (frame / 20) % 6; // Change animation every 20 frames (3 seconds)

        match cycle {
            0 => {
                // Spinning cursor animation
                let cursors = ["|", "/", "-", "\\"];
                let cursor_idx = (frame % 4) as usize;
                format!("Thinking{}  ", cursors[cursor_idx])
            }
            1 => {
                // Wave effect on letters
                let base = "Thinking...";
                let chars: Vec<char> = base.chars().collect();
                let mut result = String::new();
                for (i, &ch) in chars.iter().enumerate() {
                    let offset = (frame as i32 + i as i32 * 2) % 8;
                    if offset < 4 {
                        result.push(ch.to_ascii_uppercase());
                    } else {
                        result.push(ch.to_ascii_lowercase());
                    }
                }
                result
            }
            2 => {
                // Pulsing dots
                let dots = match frame % 4 {
                    0 => ".",
                    1 => "..",
                    2 => "...",
                    _ => "",
                };
                format!("Thinking{}", dots)
            }
            3 => {
                // Matrix-style random characters
                let base = "Thinking...";
                let chars: Vec<char> = base.chars().collect();
                let mut result = String::new();
                for &ch in &chars {
                    if (frame as usize + result.len()) % 3 == 0 {
                        // Replace with random ASCII char sometimes
                        let random_char =
                            (b'A' + ((frame as u8 + result.len() as u8) % 26)) as char;
                        result.push(random_char);
                    } else {
                        result.push(ch);
                    }
                }
                result
            }
            4 => {
                // Breathing effect with spaces
                let spaces = match (frame / 3) % 6 {
                    0 | 5 => "  ",
                    1 | 4 => " ",
                    _ => "",
                };
                format!("{}Thinking...{}", spaces, spaces)
            }
            _ => {
                // Rainbow wave effect
                let base = "Thinking...";
                let chars: Vec<char> = base.chars().collect();
                let mut result = String::new();
                for (i, &ch) in chars.iter().enumerate() {
                    let wave = ((frame as f32 * 0.3 + i as f32 * 0.5).sin() + 1.0) * 0.5;
                    if wave > 0.7 {
                        result.push(ch.to_ascii_uppercase());
                    } else if wave > 0.3 {
                        result.push(ch);
                    } else {
                        result.push(' ');
                    }
                }
                result
            }
        }
    }

    /// Get simple processing text
    pub fn get_processing_text(&self, event_count: usize) -> String {
        format!("Processing... ({} events)", event_count)
    }
}

impl Default for AnimationController {
    fn default() -> Self {
        Self::new()
    }
}
