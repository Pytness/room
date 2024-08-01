use std::collections::BTreeMap;

#[derive(Default, Debug)]
pub struct Config {
    pub ignore_case: bool,
    pub full_screen: bool,
}

impl Config {
    pub fn from_configuration(configuration: BTreeMap<String, String>) -> Self {
        let ignore_case = match configuration.get("ignore_case" as &str) {
            Some(value) => value.trim().parse().unwrap(),
            None => true,
        };

        let full_screen = match configuration.get("fullscreen" as &str) {
            Some(value) => value.trim().parse().unwrap(),
            None => false,
        };

        Self {
            ignore_case,
            full_screen,
        }
    }
}
