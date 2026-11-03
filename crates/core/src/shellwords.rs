// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! A command line as the owner types it, in words, and back. Quotes group
//! words and a backslash protects the next character, as in a shell; nothing is
//! ever expanded (`$HOME` stays `$HOME`).

/// Cuts `line` into words. `Err` says what is wrong with the quoting.
pub fn split(line: &str) -> Result<Vec<String>, &'static str> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => word.push(c),
                        None => return Err("a ' quote is never closed"),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\' | '$' | '`')) => word.push(c),
                            Some(c) => {
                                word.push('\\');
                                word.push(c);
                            }
                            None => return Err("a \" quote is never closed"),
                        },
                        Some(c) => word.push(c),
                        None => return Err("a \" quote is never closed"),
                    }
                }
            }
            '\\' => {
                in_word = true;
                word.push(chars.next().ok_or("the line ends with a backslash")?);
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

fn plain(word: &str) -> bool {
    !word.is_empty()
        && word.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '/' | '.' | '_' | '+' | '=' | ':' | ',' | '@' | '-' | '%')
        })
}

/// The words as one line that [`split`] reads back as the same words.
pub fn join<S: AsRef<str>>(words: &[S]) -> String {
    words
        .iter()
        .map(|word| {
            let word = word.as_ref();
            if plain(word) {
                word.to_owned()
            } else {
                format!("'{}'", word.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_words() {
        assert_eq!(split("  a  b\tc ").unwrap(), vec!["a", "b", "c"]);
        assert_eq!(split("").unwrap(), Vec::<String>::new());
        assert_eq!(split("   ").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn quotes_group_and_nothing_is_expanded() {
        assert_eq!(
            split(r#"/bin/x "my files" 'it''s' $HOME ~/x"#).unwrap(),
            vec!["/bin/x", "my files", "its", "$HOME", "~/x"]
        );
        assert_eq!(
            split(r#"a "b \"c\" \\ \$" d"#).unwrap(),
            vec!["a", "b \"c\" \\ $", "d"]
        );
        assert_eq!(split(r"a\ b c").unwrap(), vec!["a b", "c"]);
        assert_eq!(split(r#""" x"#).unwrap(), vec!["", "x"]);
        assert_eq!(split("'a  b'").unwrap(), vec!["a  b"]);
    }

    #[test]
    fn a_backslash_before_another_character_stays_in_double_quotes() {
        assert_eq!(split(r#""a\nb""#).unwrap(), vec![r"a\nb"]);
    }

    #[test]
    fn broken_quoting_is_explained() {
        for line in ["a 'b", "a \"b", "a\\", "\"a\\"] {
            assert!(split(line).is_err(), "{line}");
        }
    }

    #[test]
    fn what_is_joined_is_split_back() {
        let samples: [&[&str]; 5] = [
            &["/usr/bin/rsync", "-a", "/home/user/My Documents/", "/mnt/b"],
            &["x", "it's", "say \"hi\"", "$HOME", ""],
            &["a\\b", "tab\there", "new\nline"],
            &["only"],
            &[],
        ];
        for words in samples {
            let line = join(words);
            let back = split(&line).unwrap();
            let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
            assert_eq!(back, words, "{line}");
        }
    }

    #[test]
    fn plain_words_are_left_alone() {
        assert_eq!(join(&["/bin/x", "--flag=1", "50%"]), "/bin/x --flag=1 50%");
        assert_eq!(join(&["two words"]), "'two words'");
    }
}
