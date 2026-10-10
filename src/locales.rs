use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    Ru,
}

impl Locale {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "en" => Some(Locale::En),
            "ru" => Some(Locale::Ru),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ru => "ru",
        }
    }
}

#[derive(Deserialize)]
struct Translations {
    en: HashMap<String, String>,
    ru: HashMap<String, String>,
}

static TRANSLATIONS: OnceLock<Translations> = OnceLock::new();

fn translations() -> &'static Translations {
    TRANSLATIONS.get_or_init(|| {
        let raw = include_str!("locales.toml");
        let parsed: Translations = toml::from_str(raw).expect("src/locales.toml is broken");
        for key in parsed.en.keys() {
            if !parsed.ru.contains_key(key) {
                tracing::error!(key = %key, "locales.toml: key is missing in `ru`");
            }
        }
        for key in parsed.ru.keys() {
            if !parsed.en.contains_key(key) {
                tracing::error!(key = %key, "locales.toml: key is missing in `en`");
            }
        }
        parsed
    })
}

pub fn raw<'a>(l: Locale, key: &'a str) -> &'a str {
    let t = translations();
    let (primary, fallback) = match l {
        Locale::En => (&t.en, &t.ru),
        Locale::Ru => (&t.ru, &t.en),
    };
    primary
        .get(key)
        .or_else(|| fallback.get(key))
        .map(|s| s.as_str())
        .unwrap_or(key)
}

pub fn t(l: Locale, key: &str, args: &[(&str, String)]) -> String {
    let mut out = raw(l, key).to_string();
    for (name, value) in args {
        let placeholder = format!("{{{name}}}");
        out = out.replace(&placeholder, value);
    }
    out
}

#[macro_export]
macro_rules! l10n {
    ($locale:expr, $key:expr) => {
        $crate::locales::raw($locale, $key).to_string()
    };
    ($locale:expr, $key:expr, $($k:ident = $v:expr),+ $(,)?) => {
        $crate::locales::t($locale, $key, &[ $( (stringify!($k), $v.to_string()) ),+ ])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_parse() {
        assert_eq!(Locale::parse("en"), Some(Locale::En));
        assert_eq!(Locale::parse("RU"), Some(Locale::Ru));
        assert_eq!(Locale::parse("Ru"), Some(Locale::Ru));
        assert_eq!(Locale::parse("fr"), None);
        assert_eq!(Locale::parse(""), None);
    }

    #[test]
    fn test_client_details_pro_banned() {
        let text = t(
            Locale::Ru,
            "client_details",
            &[
                ("id_label", raw(Locale::Ru, "field_tg_id").to_string()),
                ("tg_id", "12345".to_string()),
                ("plan_label", raw(Locale::Ru, "field_plan").to_string()),
                ("plan", "Pro (до 31.12.2026)".to_string()),
                ("status_label", raw(Locale::Ru, "field_status").to_string()),
                (
                    "status",
                    raw(Locale::Ru, "client_status_banned").to_string(),
                ),
                ("bots_label", raw(Locale::Ru, "field_bots").to_string()),
                ("bots", "2".to_string()),
            ],
        );
        assert!(text.contains("12345"));
        assert!(text.contains("Pro (до 31.12.2026)"));
        assert!(text.contains("🚫 Забанен"));
        assert!(text.contains("2"));
    }

    #[test]
    fn test_make_by_watermark() {
        let wm = t(
            Locale::Ru,
            "make_by",
            &[("username", "@my_bot".to_string())],
        );
        assert_eq!(wm.trim(), "сделано с помощью @my_bot");
        let wm_en = t(
            Locale::En,
            "make_by",
            &[("username", "@my_bot".to_string())],
        );
        assert_eq!(wm_en.trim(), "make by @my_bot");
    }

    #[test]
    fn toml_is_valid_and_languages_are_in_sync() {
        let t = translations();
        assert!(!t.en.is_empty(), "locales.toml: `en` is empty");
        assert!(!t.ru.is_empty(), "locales.toml: `ru` is empty");
        for key in t.en.keys() {
            assert!(
                t.ru.contains_key(key),
                "locales.toml: missing `ru` key: {key}"
            );
        }
        for key in t.ru.keys() {
            assert!(
                t.en.contains_key(key),
                "locales.toml: missing `en` key: {key}"
            );
        }
    }
}
