/// Key information: evdev keycode and whether shift is required.
pub struct KeyInfo {
    pub code: u32,
    pub shift: bool,
}

// Evdev keycodes (linux/input-event-codes.h)
pub const KEY_ESC: u32 = 1;
pub const KEY_1: u32 = 2;
pub const KEY_2: u32 = 3;
pub const KEY_3: u32 = 4;
pub const KEY_4: u32 = 5;
pub const KEY_5: u32 = 6;
pub const KEY_6: u32 = 7;
pub const KEY_7: u32 = 8;
pub const KEY_8: u32 = 9;
pub const KEY_9: u32 = 10;
pub const KEY_0: u32 = 11;
pub const KEY_MINUS: u32 = 12;
pub const KEY_EQUAL: u32 = 13;
pub const KEY_TAB: u32 = 15;
pub const KEY_Q: u32 = 16;
pub const KEY_W: u32 = 17;
pub const KEY_E: u32 = 18;
pub const KEY_R: u32 = 19;
pub const KEY_T: u32 = 20;
pub const KEY_Y: u32 = 21;
pub const KEY_U: u32 = 22;
pub const KEY_I: u32 = 23;
pub const KEY_O: u32 = 24;
pub const KEY_P: u32 = 25;
pub const KEY_LEFTBRACE: u32 = 26;
pub const KEY_RIGHTBRACE: u32 = 27;
pub const KEY_ENTER: u32 = 28;
pub const KEY_LEFTCTRL: u32 = 29;
pub const KEY_A: u32 = 30;
pub const KEY_S: u32 = 31;
pub const KEY_D: u32 = 32;
pub const KEY_F: u32 = 33;
pub const KEY_G: u32 = 34;
pub const KEY_H: u32 = 35;
pub const KEY_J: u32 = 36;
pub const KEY_K: u32 = 37;
pub const KEY_L: u32 = 38;
pub const KEY_SEMICOLON: u32 = 39;
pub const KEY_APOSTROPHE: u32 = 40;
pub const KEY_GRAVE: u32 = 41;
pub const KEY_LEFTSHIFT: u32 = 42;
pub const KEY_BACKSLASH: u32 = 43;
pub const KEY_Z: u32 = 44;
pub const KEY_X: u32 = 45;
pub const KEY_C: u32 = 46;
pub const KEY_V: u32 = 47;
pub const KEY_B: u32 = 48;
pub const KEY_N: u32 = 49;
pub const KEY_M: u32 = 50;
pub const KEY_COMMA: u32 = 51;
pub const KEY_DOT: u32 = 52;
pub const KEY_SLASH: u32 = 53;
pub const KEY_LEFTALT: u32 = 56;
pub const KEY_SPACE: u32 = 57;
pub const KEY_LEFTMETA: u32 = 125;

const AZ_CODES: [u32; 26] = [
    KEY_A, KEY_B, KEY_C, KEY_D, KEY_E, KEY_F, KEY_G, KEY_H, KEY_I,
    KEY_J, KEY_K, KEY_L, KEY_M, KEY_N, KEY_O, KEY_P, KEY_Q, KEY_R,
    KEY_S, KEY_T, KEY_U, KEY_V, KEY_W, KEY_X, KEY_Y, KEY_Z,
];

/// Map a character to its evdev keycode and shift state.
pub fn char_to_key(c: char) -> Option<KeyInfo> {
    match c {
        'a'..='z' => Some(KeyInfo { code: AZ_CODES[(c as u8 - b'a') as usize], shift: false }),
        'A'..='Z' => Some(KeyInfo { code: AZ_CODES[(c as u8 - b'A') as usize], shift: true }),
        '1'..='9' => Some(KeyInfo { code: KEY_1 + (c as u32 - '1' as u32), shift: false }),
        '0' => Some(KeyInfo { code: KEY_0, shift: false }),
        ' ' => Some(KeyInfo { code: KEY_SPACE, shift: false }),
        '\n' => Some(KeyInfo { code: KEY_ENTER, shift: false }),
        '\t' => Some(KeyInfo { code: KEY_TAB, shift: false }),
        // Unshifted punctuation
        '-' => Some(KeyInfo { code: KEY_MINUS, shift: false }),
        '=' => Some(KeyInfo { code: KEY_EQUAL, shift: false }),
        '[' => Some(KeyInfo { code: KEY_LEFTBRACE, shift: false }),
        ']' => Some(KeyInfo { code: KEY_RIGHTBRACE, shift: false }),
        '\\' => Some(KeyInfo { code: KEY_BACKSLASH, shift: false }),
        ';' => Some(KeyInfo { code: KEY_SEMICOLON, shift: false }),
        '\'' => Some(KeyInfo { code: KEY_APOSTROPHE, shift: false }),
        '`' => Some(KeyInfo { code: KEY_GRAVE, shift: false }),
        ',' => Some(KeyInfo { code: KEY_COMMA, shift: false }),
        '.' => Some(KeyInfo { code: KEY_DOT, shift: false }),
        '/' => Some(KeyInfo { code: KEY_SLASH, shift: false }),
        // Shifted punctuation
        '!' => Some(KeyInfo { code: KEY_1, shift: true }),
        '@' => Some(KeyInfo { code: KEY_2, shift: true }),
        '#' => Some(KeyInfo { code: KEY_3, shift: true }),
        '$' => Some(KeyInfo { code: KEY_4, shift: true }),
        '%' => Some(KeyInfo { code: KEY_5, shift: true }),
        '^' => Some(KeyInfo { code: KEY_6, shift: true }),
        '&' => Some(KeyInfo { code: KEY_7, shift: true }),
        '*' => Some(KeyInfo { code: KEY_8, shift: true }),
        '(' => Some(KeyInfo { code: KEY_9, shift: true }),
        ')' => Some(KeyInfo { code: KEY_0, shift: true }),
        '_' => Some(KeyInfo { code: KEY_MINUS, shift: true }),
        '+' => Some(KeyInfo { code: KEY_EQUAL, shift: true }),
        '{' => Some(KeyInfo { code: KEY_LEFTBRACE, shift: true }),
        '}' => Some(KeyInfo { code: KEY_RIGHTBRACE, shift: true }),
        '|' => Some(KeyInfo { code: KEY_BACKSLASH, shift: true }),
        ':' => Some(KeyInfo { code: KEY_SEMICOLON, shift: true }),
        '"' => Some(KeyInfo { code: KEY_APOSTROPHE, shift: true }),
        '~' => Some(KeyInfo { code: KEY_GRAVE, shift: true }),
        '<' => Some(KeyInfo { code: KEY_COMMA, shift: true }),
        '>' => Some(KeyInfo { code: KEY_DOT, shift: true }),
        '?' => Some(KeyInfo { code: KEY_SLASH, shift: true }),
        _ => None,
    }
}

/// Parse a key combo string like "ctrl+v", "enter", "shift+a".
/// Returns (modifier_keycodes, final_keycode).
pub fn parse_combo(combo: &str) -> Result<(Vec<u32>, u32), String> {
    let parts: Vec<&str> = combo.split('+').collect();
    if parts.is_empty() {
        return Err("empty combo".into());
    }

    let mut modifiers = Vec::new();

    // All parts except last are modifiers
    for &part in &parts[..parts.len() - 1] {
        let modifier = match part.to_lowercase().as_str() {
            "ctrl" | "control" => KEY_LEFTCTRL,
            "shift" => KEY_LEFTSHIFT,
            "alt" => KEY_LEFTALT,
            "super" | "meta" => KEY_LEFTMETA,
            other => return Err(format!("unknown modifier '{}'", other)),
        };
        modifiers.push(modifier);
    }

    // Last part is the key
    let key_str = parts.last().unwrap().to_lowercase();
    let keycode = if key_str.len() == 1 {
        let c = key_str.chars().next().unwrap();
        let ki = char_to_key(c).ok_or_else(|| format!("unknown key '{}'", c))?;
        if ki.shift && !modifiers.contains(&KEY_LEFTSHIFT) {
            modifiers.push(KEY_LEFTSHIFT);
        }
        ki.code
    } else {
        match key_str.as_str() {
            "enter" | "return" => KEY_ENTER,
            "tab" => KEY_TAB,
            "space" => KEY_SPACE,
            "esc" | "escape" => KEY_ESC,
            other => return Err(format!("unknown key '{}'", other)),
        }
    };

    Ok((modifiers, keycode))
}
