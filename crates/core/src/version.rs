// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! Version numbers, the website's `latest.json` and the release notes of
//! `CHANGELOG.md`, for the update notices.

/// Seconds between two checks for a new version.
pub const CHECK_INTERVAL: i64 = 24 * 60 * 60;

/// True when `candidate` is a newer version than `current` (`0.10.0` > `0.9.1`).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    version_parts(candidate) > version_parts(current)
}

/// `v1.2.3-rc1` → `[1, 2, 3]`; missing or unreadable parts count as 0.
fn version_parts(version: &str) -> [u64; 3] {
    let mut parts = [0; 3];
    let core = version.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next().unwrap_or_default();
    for (slot, part) in parts.iter_mut().zip(core.split('.')) {
        *slot = part.parse().unwrap_or(0);
    }
    parts
}

/// True when the last check is old enough (or in the future: clock changed).
pub fn check_due(last_check: i64, now: i64) -> bool {
    now < last_check || now - last_check >= CHECK_INTERVAL
}

/// The `version` of the website's `latest.json` (`{"version": "0.2.0", …}`).
pub fn latest_version(json: &str) -> Option<String> {
    let after_key = &json[json.find("\"version\"")? + "\"version\"".len()..];
    let value = after_key.trim_start().strip_prefix(':')?.trim_start();
    let value = value.strip_prefix('"')?;
    let version = &value[..value.find('"')?];
    let plain = !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    plain.then(|| version.to_owned())
}

/// Version and notes of the newest section of `CHANGELOG.md`
/// (`## 0.2.0 (2026-12-01)`), as the AppStream markup `AdwAboutDialog` shows:
/// paragraphs in `<p>`, `- ` items in one `<ul>`.
pub fn release_notes(changelog: &str) -> Option<(String, String)> {
    let start = changelog.find("\n## ")? + 1;
    let section = &changelog[start..];
    let (heading, body) = section.split_once('\n')?;
    let version = heading
        .trim_start_matches("## ")
        .split(' ')
        .next()?
        .to_owned();
    let body = body.split("\n## ").next().unwrap_or_default();

    let mut notes = String::new();
    let mut paragraph = String::new();
    let mut items: Vec<String> = Vec::new();
    let flush_paragraph = |notes: &mut String, paragraph: &mut String| {
        if !paragraph.is_empty() {
            notes.push_str(&format!("<p>{}</p>", escape(paragraph)));
            paragraph.clear();
        }
    };
    let flush_items = |notes: &mut String, items: &mut Vec<String>| {
        if !items.is_empty() {
            notes.push_str("<ul>");
            for item in items.drain(..) {
                notes.push_str(&format!("<li>{}</li>", escape(&item)));
            }
            notes.push_str("</ul>");
        }
    };
    for line in body.lines() {
        let text = line.trim();
        if text.is_empty() {
            flush_paragraph(&mut notes, &mut paragraph);
        } else if let Some(item) = text.strip_prefix("- ") {
            flush_paragraph(&mut notes, &mut paragraph);
            items.push(item.to_owned());
        } else if line.starts_with(' ') && !items.is_empty() {
            // The next line of a wrapped item.
            if let Some(last) = items.last_mut() {
                last.push(' ');
                last.push_str(text);
            }
        } else {
            flush_items(&mut notes, &mut items);
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(text);
        }
    }
    flush_paragraph(&mut notes, &mut paragraph);
    flush_items(&mut notes, &mut items);
    Some((version, notes))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.10.0", "0.9.1"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0-rc1", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn checks_once_a_day() {
        assert!(check_due(0, CHECK_INTERVAL));
        assert!(!check_due(1000, 1000 + CHECK_INTERVAL - 1));
        // The clock went back: check rather than wait for a date in the future.
        assert!(check_due(5000, 10));
    }

    #[test]
    fn the_latest_version_is_read_from_the_json() {
        let json = r#"{ "version" : "0.2.0", "date": "2026-12-01" }"#;
        assert_eq!(latest_version(json).as_deref(), Some("0.2.0"));
        assert_eq!(
            latest_version(r#"{"version":"1.0.0-rc1"}"#).as_deref(),
            Some("1.0.0-rc1")
        );
        for wrong in [
            r#"{}"#,
            r#"{"version": 2}"#,
            r#"{"version": "<b>"}"#,
            "not json",
        ] {
            assert_eq!(latest_version(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn whats_new_comes_from_the_newest_changelog_section() {
        let changelog = "# Changelog\n\nIntro.\n\n## 0.2.0 (2026-12-01)\n\nFaster & <safer>.\n\n- First item,\n  on two lines.\n- `Second` item.\n\n## 0.1.0 (2026-11-11)\n\nFirst.\n";
        let (version, notes) = release_notes(changelog).expect("a section");
        assert_eq!(version, "0.2.0");
        assert_eq!(
            notes,
            "<p>Faster &amp; &lt;safer&gt;.</p><ul><li>First item, on two lines.</li><li>Second item.</li></ul>"
        );
    }

    #[test]
    fn the_real_changelog_describes_this_version() {
        let (version, notes) =
            release_notes(include_str!("../../../CHANGELOG.md")).expect("a section");
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
        assert!(notes.starts_with("<p>") && notes.contains("<li>"));
    }
}
