use std::{borrow::Cow, char, collections::HashMap, env};

use fancy_regex::{Captures, Regex};
use lazy_static::lazy_static;
use thiserror::Error;

#[derive(Error, Debug, Clone, Copy)]
pub enum Error {
    #[error("could not find varible")]
    NotPresent,

    #[error("varible name is not valid")]
    InvalidName,

    #[error("varible contains a bad value")]
    InvalidValue,
}

#[derive(Debug, Clone)]
pub enum OnNotPresent {
    Error,
    Passthrough,
    Default(String),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub pattern: Regex,
    pub on_not_present: OnNotPresent,
    pub escape: Vec<(&'static str, &'static str)>,
}

struct Pattern {
    form: Regex,
    close: Option<String>,
}

impl Config {
    fn bash() -> Config {
        Config {
            on_not_present: OnNotPresent::Default("".to_string()),
            pattern: Regex::new(
                // 1st ?: -> ?|
                r"(?<!\\)\$(?:(?<name>\w+)|(?:{(?<name>\w+)(?:-(?<default>\w+))?}))",
            )
            .unwrap(),
            escape: vec![("\\$", "$")],
        }
    }
    fn docker() -> Config {
        Config {
            on_not_present: OnNotPresent::Default("".to_string()),
            pattern: Regex::new(r"(?<!\$)\$(?<name>\w+)").unwrap(),
            escape: vec![("$", "$$")],
        }
    }
    fn cmd() -> Config {
        Config {
            on_not_present: OnNotPresent::Default("".to_string()),
            pattern: Regex::new(r"(?<!\%)%(?<name>\w+)%").unwrap(),
            escape: vec![("$", "$$")],
        }
    }
}
lazy_static! {
    static ref BASH: Config = Config::bash();
}

trait ValueProivder {
    fn get(&self, name: impl AsRef<str>) -> Result<Cow<str>, Error>;

    fn substitute<'t>(&self, config: &'t Config, on: &'t str) -> Result<Cow<'t, str>, Error> {
        let mut errors = Vec::new();

        let replacer = |captures: &Captures| {
            let captures: Vec<_> = captures.iter().flatten().collect();
            let all = captures.get(0);
            let name = captures
                .get(1) //name
                .expect("invalid regex doesn't capture name")
                .as_str();
            let default = captures.get(2);
            let value = self.get(name);
            let (error, result) = match value {
                Ok(result) => (None, result),
                Err(error) => match error {
                    Error::NotPresent => {
                        if let Some(default) = default {
                            //(None, default)
                            (None, Cow::Owned(default.as_str().to_string()))
                        } else {
                            match &config.on_not_present {
                                OnNotPresent::Error => (Some(Error::NotPresent), Cow::Borrowed("")),
                                OnNotPresent::Passthrough => (
                                    None,
                                    Cow::Owned(
                                        all
                                            .expect("match didn't match")
                                            .as_str()
                                            .to_string(),
                                    ),
                                ),
                                OnNotPresent::Default(_default) => {
                                    //default.as_str()
                                    (None, Cow::Borrowed(""))
                                }
                            }
                        }
                    }
                    error => (Some(error), Cow::Borrowed("")),
                },
            };

            if let Some(error) = error {
                errors.push(error);
            }
            result /*
                   let value = value.unwrap();
                   value
                           */
        };
        let replaced = config.pattern.replace_all(on, replacer);
        if let Some(error) = errors.pop() {
            Err(error)
        } else {
            Ok(replaced)
        }
    }
}

pub struct Env;

impl Env {
    fn invalid_varible_name_pattern(char: char) -> bool {
        char == '=' || char == '\0'
    }
}
impl ValueProivder for Env {
    fn get(&self, name: impl AsRef<str>) -> Result<Cow<str>, Error> {
        let name = name.as_ref();
        if name.contains(Env::invalid_varible_name_pattern) {
            Err(Error::InvalidName)
        } else {
            match env::var(name) {
                Ok(value) => Ok(Cow::Owned(value)),
                Err(error) => match error {
                    env::VarError::NotPresent => Err(Error::NotPresent),
                    env::VarError::NotUnicode(_) => Err(Error::InvalidValue),
                },
            }
        }
    }
}

impl ValueProivder for HashMap<String, String> {
    fn get(&self, name: impl AsRef<str>) -> Result<Cow<str>, Error> {
        match self.get(name.as_ref()) {
            Some(value) => Ok(Cow::Borrowed(value)),
            None => Err(Error::NotPresent),
        }
    }
}

pub fn env(expand: &str) -> Result<Cow<str>, Error> {
    Env.substitute(&BASH, expand.as_ref())
}

// fn substitute(string: , impl ValueProivder) -> String {
//     if name.as_ref().contains(is_invalid_env_character) {
//         Err(ProviderError::InvalidValue)
//     } else {
//     }
// }

#[cfg(test)]
mod tests {
    use regex::RegexSet;

    use crate::env;

    #[test]
    fn stuff() {
        let value = env!("PWD");
        assert_eq!(env("${PWD}").expect("failed to expand").as_ref(), value);
        assert_eq!(env("$PWD").expect("failed to expand").as_ref(), value);
    }

    #[test]
    fn regex_set() {
        // this is after split into words
        let set = RegexSet::new(&[
            // single quote
            r"'[^']+'",
            // double quote
            r#""(?:\\")|(?:[^'])+""#,
            //tilde Expansion
            r"`((\\`)|[^`])+`",
            //Parameter Expansion
            r"\$\{.*\}",
            // Arithmetic Expansion
            r"\$\(\(.+\)\)",
            // Command Substitution
            r"\$\(.+\)",
            // escape
            r"\\.+",
            r"bar",
            r"barfoo",
            r"foobar",
        ])
        .unwrap();

        // after this we do field expansion

        let matches = set.matches("foobar");
    }
}
