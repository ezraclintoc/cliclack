use std::io;
use std::{fmt::Display, str::FromStr};

use console::Key;

use crate::{
    autocomplete::{Autocomplete},
    prompt::{
        cursor::StringCursor,
        interaction::{Event, PromptInteraction, State},
    },
    theme::THEME,
    validate::Validate,
};

type ValidationCallback = Box<dyn Fn(&String) -> Result<(), String>>;

#[derive(Default, PartialEq)]
enum Multiline {
    #[default]
    Disabled,
    Preview,
    Editing,
}

/// A prompt that accepts a text input: either single-line or multiline.
///
/// # Example
///
/// ```
/// use cliclack::Input;
///
/// # fn test() -> std::io::Result<()> {
/// let input: String = Input::new("Tea or coffee?")
///     .placeholder("Yes")
///     .interact()?;
/// # Ok(())
/// # }
/// # test().ok();
/// ```
///
/// # Multiline
///
/// [`Input::multiline`] enables multiline text editing.
///
/// ```
/// use cliclack::Input;
///
/// # fn test() -> std::io::Result<()> {
/// let path: String = Input::new("Input multiple lines: ")
///     .multiline()
///     .interact()?;
/// # Ok(())
/// # }
/// # test().ok(); // Ignoring I/O runtime errors.
/// ```
#[derive(Default)]
pub struct Input {
    prompt: String,
    input: StringCursor,
    input_required: bool,
    default: Option<String>,
    placeholder: StringCursor,
    multiline: Multiline,
    validate_on_enter: Option<ValidationCallback>,
    validate_interactively: Option<ValidationCallback>,
    autocompleter: Option<Box<dyn Autocomplete>>,
    autocompletion_index: Option<usize>,
    autocompletion_query: String,
    autocomplete_on_enter: bool,
}

impl Input {
    /// Creates a new input prompt.
    pub fn new(prompt: impl Display) -> Self {
        Self {
            prompt: prompt.to_string(),
            input_required: true,
            ..Default::default()
        }
    }

    /// Sets the placeholder (hint) text for the input.
    pub fn placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder.extend(placeholder);
        self
    }

    /// Sets the default value for the input and also a hint (placeholder) if one is not already set.
    ///
    /// [`Input::placeholder`] overrides a hint set by `default()`, however, default value
    /// is used is no value has been supplied.
    pub fn default_input(mut self, value: &str) -> Self {
        self.default = Some(value.into());
        self
    }

    /// Sets whether the input is required. Default: `true`.
    ///
    /// [`Input::default_input`] is used if no value is supplied.
    pub fn required(mut self, required: bool) -> Self {
        self.input_required = required;
        self
    }

    /// Enables multiline input.
    ///
    /// 1. Press `Esc` to review and submit.
    /// 2. Start typing to get back into the editing mode.
    pub fn multiline(mut self) -> Self {
        self.multiline = Multiline::Editing;
        self
    }

    /// Sets a validation callback for the input that is called when the user submits.
    /// The same as [`Input::validate_on_enter`].
    pub fn validate<V>(mut self, validator: V) -> Self
    where
        V: Validate<String> + 'static,
        V::Err: ToString,
    {
        self.validate_on_enter = Some(Box::new(move |input: &String| {
            validator.validate(input).map_err(|err| err.to_string())
        }));
        self
    }

    /// Sets a validation callback for the input that is called when the user submits.
    pub fn validate_on_enter<V>(self, validator: V) -> Self
    where
        V: Validate<String> + 'static,
        V::Err: ToString,
    {
        self.validate(validator)
    }

    /// Validates input while user is typing.
    pub fn validate_interactively<V>(mut self, validator: V) -> Self
    where
        V: Validate<String> + 'static,
        V::Err: ToString,
    {
        self.validate_interactively = Some(Box::new(move |input: &String| {
            validator.validate(input).map_err(|err| err.to_string())
        }));
        self
    }

    /// Starts the prompt interaction.
    pub fn interact<T>(&mut self) -> io::Result<T>
    where
        T: FromStr,
    {
        if self.placeholder.is_empty() {
            if let Some(default) = &self.default {
                self.placeholder.extend(default);
                self.placeholder.extend(" (default)");

                if self.multiline == Multiline::Editing {
                    self.multiline = Multiline::Preview;
                }
            }
        }
        <Self as PromptInteraction<T>>::interact(self)
    }

    /// Sets a list of suggestions for autocompletion.
    ///
    /// When the user presses Tab or uses arrow keys, they can cycle through
    /// matching suggestions.
    pub fn autocomplete(mut self, suggestions: Vec<String>) -> Self {
        self.autocompleter = Some(Box::new(suggestions));
        self
    }

    /// Sets a dynamic autocomplete handler function.
    ///
    /// The handler is called with the current input to get suggestions on each keystroke.
    pub fn autocomplete_with<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> crate::autocomplete::AutocompleteResult + Send + 'static,
    {
        self.autocompleter = Some(Box::new(handler));
        self
    }

    /// Enables auto-selecting the first suggestion when pressing Enter.
    ///
    /// If there are matching suggestions, the first one will be automatically
    /// selected and filled in when the user presses Enter.
    pub fn autocomplete_on_enter(mut self) -> Self {
        self.autocomplete_on_enter = true;
        self
    }

    fn get_filtered_suggestions(&mut self, query: &str) -> Vec<String> {
        if let Some(ref mut completer) = self.autocompleter {
            match completer.get_suggestions(query) {
                Ok(suggestions) => suggestions,
                Err(_) => vec![],
            }
        } else {
            vec![]
        }
    }
}

impl<T> PromptInteraction<T> for Input
where
    T: FromStr,
{
    fn input(&mut self) -> Option<&mut StringCursor> {
        if self.multiline == Multiline::Preview {
            return None;
        }
        Some(&mut self.input)
    }

    fn on(&mut self, event: &Event) -> State<T> {
        let Event::Key(key) = event;
        let mut submit = false;

        let query = self.input.to_string();
        let filter_query = if self.autocompletion_query.is_empty() {
            query.clone()
        } else {
            self.autocompletion_query.clone()
        };

        match key {
            Key::Tab if self.autocompleter.is_some() => {
                let filtered_suggestions = self.get_filtered_suggestions(&filter_query);
                if filtered_suggestions.is_empty() {
                    return State::Active;
                }

                if self.autocompletion_query.is_empty() {
                    self.autocompletion_query = query.clone();
                }

                let new_index = match self.autocompletion_index {
                    None => Some(0),
                    Some(idx) => {
                        if idx >= filtered_suggestions.len() - 1 {
                            None
                        } else {
                            Some(idx + 1)
                        }
                    }
                };
                self.autocompletion_index = new_index;
                if let Some(idx) = self.autocompletion_index {
                    self.input.clear();
                    self.input.extend(&filtered_suggestions[idx]);
                }
                return State::Active;
            }
            Key::ArrowDown if self.autocompleter.is_some() => {
                let filtered_suggestions = self.get_filtered_suggestions(&filter_query);
                if filtered_suggestions.is_empty() {
                    return State::Active;
                }

                if self.autocompletion_query.is_empty() {
                    self.autocompletion_query = query.clone();
                }

                let new_index = match self.autocompletion_index {
                    None => Some(0),
                    Some(idx) => {
                        if idx >= filtered_suggestions.len() - 1 {
                            Some(0)
                        } else {
                            Some(idx + 1)
                        }
                    }
                };
                self.autocompletion_index = new_index;
                if let Some(idx) = self.autocompletion_index {
                    self.input.clear();
                    self.input.extend(&filtered_suggestions[idx]);
                }
                return State::Active;
            }
            Key::ArrowUp if self.autocompleter.is_some() => {
                let filtered_suggestions = self.get_filtered_suggestions(&filter_query);
                if filtered_suggestions.is_empty() {
                    return State::Active;
                }

                if self.autocompletion_query.is_empty() {
                    self.autocompletion_query = query.clone();
                }

                let new_index = match self.autocompletion_index {
                    None => Some(filtered_suggestions.len() - 1),
                    Some(idx) => {
                        if idx == 0 {
                            Some(filtered_suggestions.len() - 1)
                        } else {
                            Some(idx - 1)
                        }
                    }
                };
                self.autocompletion_index = new_index;
                if let Some(idx) = self.autocompletion_index {
                    self.input.clear();
                    self.input.extend(&filtered_suggestions[idx]);
                }
                return State::Active;
            }
            Key::Escape if self.multiline == Multiline::Editing => {
                self.multiline = Multiline::Preview;
                return State::Cancel;
            }
            Key::Enter => {
                if self.multiline == Multiline::Editing {
                    self.input.insert('\n')
                } else {
                    submit = true;
                }
            }
            Key::Char(c) if !c.is_ascii_control() && self.multiline == Multiline::Preview => {
                self.input.insert(*c);
            }
            Key::Backspace if self.multiline == Multiline::Preview => self.input.delete_left(),
            Key::Char(c) if !c.is_ascii_control() => {
                self.autocompletion_index = None;
                self.autocompletion_query.clear();
            }
            Key::Backspace => {
                self.autocompletion_index = None;
                self.autocompletion_query.clear();
            }
            _ => {}
        }

        if submit && self.autocomplete_on_enter && self.autocompletion_index.is_none() {
            let suggestions = self.get_filtered_suggestions(&self.input.to_string());
            if !suggestions.is_empty() {
                self.input.clear();
                self.input.extend(&suggestions[0]);
            }
        }

        if self.multiline == Multiline::Preview {
            self.multiline = Multiline::Editing;
        }

        if submit && self.input.is_empty() {
            if let Some(default) = &self.default {
                self.input.extend(default);
            } else if self.input_required {
                return State::Error("Input required".to_string());
            }
        }

        if let Some(validator) = &self.validate_interactively {
            if let Err(err) = validator(&self.input.to_string()) {
                return State::Error(err);
            }

            if self.input.to_string().parse::<T>().is_err() {
                return State::Error("Invalid value format".to_string());
            }
        }

        if submit {
            if let Some(validator) = &self.validate_on_enter {
                if let Err(err) = validator(&self.input.to_string()) {
                    return State::Error(err);
                }
            }

            match self.input.to_string().parse::<T>() {
                Ok(value) => return State::Submit(value),
                Err(_) => return State::Error("Invalid value format".to_string()),
            }
        }

        State::Active
    }

    fn render(&mut self, state: &State<T>) -> String {
        let theme = THEME.read().unwrap();

        let filter_query = if self.autocompletion_query.is_empty() {
            self.input.to_string()
        } else {
            self.autocompletion_query.clone()
        };

        let filtered_suggestions: Vec<String> = self.get_filtered_suggestions(&filter_query);

        let suggestions = if !matches!(state, State::Active) {
            String::new()
        } else if filtered_suggestions.is_empty() {
            String::new()
        } else {
            let suggestions_text = filtered_suggestions
                .iter()
                .enumerate()
                .map(|(i, choice)| {
                    let is_selected = self.autocompletion_index.map_or(false, |idx| idx == i);
                    theme.format_autocomplete_item(&state.into(), choice, is_selected)
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{}\n", suggestions_text)
        };

        let prompt = theme.format_header(&state.into(), &self.prompt);
        let input = if self.input.is_empty() {
            theme.format_placeholder(&state.into(), &self.placeholder)
        } else {
            theme.format_input(&state.into(), &self.input)
        };

        let mut footer_message = match self.multiline {
            Multiline::Editing => "[Esc](Preview)",
            Multiline::Preview => "[Enter](Submit)",
            _ => "",
        };

        if self.autocompleter.is_some() && !filtered_suggestions.is_empty() {
            footer_message = "";
        }

        let footer = theme.format_footer_with_message(&state.into(), footer_message);

        let footer = if matches!(state, State::Active)
            && self.autocompleter.is_some()
            && !filtered_suggestions.is_empty()
        {
            theme.format_footer_with_message(&state.into(), "\r└ ◇")
        } else {
            footer
        };

        prompt + &input + &footer + &suggestions
    }
}
