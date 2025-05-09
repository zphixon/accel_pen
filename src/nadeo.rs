use std::collections::HashSet;

use crate::error::ApiError;
use base64::Engine;
use serde::Serialize;
use ts_rs::TS;
use uuid::Uuid;

pub mod api;
pub mod auth;

pub fn login_to_account_id(login: &str) -> Result<String, ApiError> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(login)?;
    let hex_string = hex::encode(bytes);
    let uuid = Uuid::try_parse(&hex_string)?;
    Ok(uuid.hyphenated().to_string())
}

pub fn account_id_to_login(account_id: &str) -> Result<String, ApiError> {
    let _uuid = Uuid::parse_str(account_id)?;
    let bytes = hex::decode(account_id.replace("-", "")).expect("UUID not made of hex digits");
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

#[derive(Serialize, Clone, Copy, TS)]
#[ts(export)]
pub struct Color {
    pub r: char,
    pub g: char,
    pub b: char,
}

#[derive(Serialize, PartialEq, Eq, Hash, Clone, Copy, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Format {
    TextBold,
    TextItalic,
    TextStretch,
    TextShrink,
    TextUppercase,
    TextShadow,
}

impl Format {
    fn from_char(char: char) -> Option<Self> {
        match char {
            'o' => Some(Format::TextBold),
            'i' => Some(Format::TextItalic),
            'w' => Some(Format::TextStretch),
            'n' => Some(Format::TextShrink),
            't' => Some(Format::TextUppercase),
            's' => Some(Format::TextShadow),
            _ => None
        }
    }
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct FormattedChar {
    pub char: char,
    pub icon: bool,
    pub color: Option<Color>,
    pub format: HashSet<Format>,
}

impl FormattedChar {
    fn icon(char: char, color: Option<Color>, format: &HashSet<Format>) -> Self {
        FormattedChar { char, icon: true, color, format: format.clone() }
    }

    fn char(char: char, color: Option<Color>, format: &HashSet<Format>) -> Self {
        FormattedChar { char, icon: false, color, format: format.clone() }
    }
}

pub fn to_formatted_string(s: &str) -> Vec<FormattedChar> {
    let mut formatted = Vec::new();

    let mut format = HashSet::new();
    let mut color = None::<Color>;

    let mut i = 0;
    let chars = s.chars().collect::<Vec<_>>();
    while i < chars.len() {
        let char = chars[i];
        let codepoint = chars[i] as u32;
        if 0xE000 <= codepoint && codepoint <= 0xF8FF
            || 0xF0000 <= codepoint && codepoint <= 0xFFFFD
            || 0x100000 <= codepoint && codepoint <= 0x10FFFD
        {
            i += 1;
            formatted.push(FormattedChar::icon(char, color, &format));
        } else if char == '$' {
            i += 1;
            let Some(&code) = chars.get(i) else {
                break;
            };
            if code == '$' {
                formatted.push(FormattedChar::char('$', color, &format));
            } else if let Some(format_char) = Format::from_char(code) {
                format.insert(format_char);
            } else if code == 'g' {
                color = None;
            } else if code == 'z' {
                format.clear();
            } else {
                let r = code;
                i += 1;
                let Some(&g) = chars.get(i) else {
                    break;
                };
                i += 1;
                let Some(&b) = chars.get(i) else {
                    break;
                };
                color = Some(Color { r, g, b })
            }
            i += 1;
        } else {
            i += 1;
            formatted.push(FormattedChar::char(char, color, &format));
        }
    }

    formatted
}

pub fn strip_formatting(formatted: &[FormattedChar]) -> String {
    formatted.iter().map(|format| format.char).collect()
}
