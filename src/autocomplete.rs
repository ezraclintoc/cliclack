use std::path::PathBuf;

use crate::filter::filter_strings;

pub type AutocompleteResult = Result<Vec<String>, String>;

pub trait Autocomplete: Send {
    fn get_suggestions(&mut self, input: &str) -> AutocompleteResult;
    fn get_completion(&mut self, input: &str, highlighted: Option<String>) -> Option<String>;
}

impl Autocomplete for Vec<String> {
    fn get_suggestions(&mut self, input: &str) -> AutocompleteResult {
        Ok(filter_strings(input, self))
    }

    fn get_completion(&mut self, _input: &str, highlighted: Option<String>) -> Option<String> {
        highlighted
    }
}

impl<F> Autocomplete for F
where
    F: Fn(&str) -> AutocompleteResult + Send,
{
    fn get_suggestions(&mut self, input: &str) -> AutocompleteResult {
        self(input)
    }

    fn get_completion(&mut self, _input: &str, highlighted: Option<String>) -> Option<String> {
        highlighted
    }
}