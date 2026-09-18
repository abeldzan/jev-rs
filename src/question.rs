use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{Error, JsonContent};

/// Optional descriptions for the positive and negative outcomes of a noul.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoulCriteria {
    yes: Option<Option<JsonContent>>,
    no: Option<Option<JsonContent>>,
}

impl NoulCriteria {
    /// Creates criteria with both outcomes absent.
    pub fn new() -> Self {
        Self::default()
    }

    /// Describes the positive outcome.
    pub fn yes(mut self, value: impl Into<JsonContent>) -> Self {
        self.yes = Some(Some(value.into()));
        self
    }

    /// Sends an explicit JSON null for the positive outcome.
    pub fn yes_null(mut self) -> Self {
        self.yes = Some(None);
        self
    }

    /// Describes the negative outcome.
    pub fn no(mut self, value: impl Into<JsonContent>) -> Self {
        self.no = Some(Some(value.into()));
        self
    }

    /// Sends an explicit JSON null for the negative outcome.
    pub fn no_null(mut self) -> Self {
        self.no = Some(None);
        self
    }

    fn into_value(self) -> Value {
        let mut map = Map::new();
        if let Some(value) = self.yes {
            map.insert(
                "true".to_owned(),
                value.map(JsonContent::into_value).unwrap_or(Value::Null),
            );
        }
        if let Some(value) = self.no {
            map.insert(
                "false".to_owned(),
                value.map(JsonContent::into_value).unwrap_or(Value::Null),
            );
        }
        Value::Object(map)
    }
}

/// A yes/no question.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Noul {
    instructions: Option<JsonContent>,
    criteria: Option<NoulCriteria>,
}

impl Noul {
    /// Creates a noul question with instructions.
    pub fn new(instructions: impl Into<JsonContent>) -> Self {
        Self {
            instructions: Some(instructions.into()),
            criteria: None,
        }
    }

    /// Creates a noul question without instructions.
    pub fn bare() -> Self {
        Self::default()
    }

    /// Sets the question instructions.
    pub fn instructions(mut self, value: impl Into<JsonContent>) -> Self {
        self.instructions = Some(value.into());
        self
    }

    /// Sets optional outcome descriptions.
    pub fn criteria(mut self, value: NoulCriteria) -> Self {
        self.criteria = Some(value);
        self
    }

    /// Describes the positive outcome.
    pub fn yes_description(mut self, value: impl Into<JsonContent>) -> Self {
        self.criteria = Some(self.criteria.unwrap_or_default().yes(value));
        self
    }

    /// Sends an explicit JSON null for the positive outcome.
    pub fn yes_null(mut self) -> Self {
        self.criteria = Some(self.criteria.unwrap_or_default().yes_null());
        self
    }

    /// Describes the negative outcome.
    pub fn no_description(mut self, value: impl Into<JsonContent>) -> Self {
        self.criteria = Some(self.criteria.unwrap_or_default().no(value));
        self
    }

    /// Sends an explicit JSON null for the negative outcome.
    pub fn no_null(mut self) -> Self {
        self.criteria = Some(self.criteria.unwrap_or_default().no_null());
        self
    }
}

/// A question selecting one named alternative.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    instructions: Option<JsonContent>,
    criteria: IndexMap<String, Option<JsonContent>>,
}

impl Choice {
    /// Creates a choice question with instructions and no options yet.
    pub fn new(instructions: impl Into<JsonContent>) -> Self {
        Self {
            instructions: Some(instructions.into()),
            criteria: IndexMap::new(),
        }
    }

    /// Creates a choice question without instructions.
    pub fn bare() -> Self {
        Self {
            instructions: None,
            criteria: IndexMap::new(),
        }
    }

    /// Sets the question instructions.
    pub fn instructions(mut self, value: impl Into<JsonContent>) -> Self {
        self.instructions = Some(value.into());
        self
    }

    /// Adds undescribed options in iteration order.
    pub fn options<K, I>(mut self, options: I) -> Self
    where
        K: Into<String>,
        I: IntoIterator<Item = K>,
    {
        self.criteria
            .extend(options.into_iter().map(|label| (label.into(), None)));
        self
    }

    /// Adds one undescribed option.
    pub fn option(mut self, label: impl Into<String>) -> Self {
        self.criteria.insert(label.into(), None);
        self
    }

    /// Adds one option with a description.
    pub fn option_with_description(
        mut self,
        label: impl Into<String>,
        description: impl Into<JsonContent>,
    ) -> Self {
        self.criteria.insert(label.into(), Some(description.into()));
        self
    }
}

/// A question assigning a score from an ordered rubric.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    instructions: Option<JsonContent>,
    criteria: Vec<JsonContent>,
}

impl Score {
    /// Creates a score question with instructions and no levels yet.
    pub fn new(instructions: impl Into<JsonContent>) -> Self {
        Self {
            instructions: Some(instructions.into()),
            criteria: Vec::new(),
        }
    }

    /// Creates a score question without instructions.
    pub fn bare() -> Self {
        Self {
            instructions: None,
            criteria: Vec::new(),
        }
    }

    /// Sets the question instructions.
    pub fn instructions(mut self, value: impl Into<JsonContent>) -> Self {
        self.instructions = Some(value.into());
        self
    }

    /// Adds ordered rubric levels.
    pub fn levels<I, V>(mut self, levels: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<JsonContent>,
    {
        self.criteria.extend(levels.into_iter().map(Into::into));
        self
    }

    /// Adds one rubric level.
    pub fn level(mut self, level: impl Into<JsonContent>) -> Self {
        self.criteria.push(level.into());
        self
    }
}

/// A typed or forward-compatible raw System One question.
#[derive(Clone, Debug, PartialEq)]
pub enum Question {
    /// Yes/no question.
    Noul(Noul),
    /// Named-alternative question.
    Choice(Choice),
    /// Ordered-rubric question.
    Score(Score),
    /// Pass-through JSON question object.
    Raw(Map<String, Value>),
}

impl Question {
    /// Wraps a raw question object.
    pub fn raw(value: Map<String, Value>) -> Self {
        Self::Raw(value)
    }

    pub(crate) fn into_wire(self, name: &str) -> Result<Value, Error> {
        let mut map = Map::new();
        match self {
            Self::Noul(question) => {
                map.insert("type".into(), Value::String("noul".into()));
                insert_instructions(&mut map, question.instructions);
                if let Some(criteria) = question.criteria {
                    map.insert("criteria".into(), criteria.into_value());
                }
            }
            Self::Choice(question) => {
                map.insert("type".into(), Value::String("choice".into()));
                insert_instructions(&mut map, question.instructions);
                map.insert(
                    "criteria".into(),
                    Value::Object(
                        question
                            .criteria
                            .into_iter()
                            .map(|(label, description)| {
                                (
                                    label,
                                    description
                                        .map(JsonContent::into_value)
                                        .unwrap_or(Value::Null),
                                )
                            })
                            .collect(),
                    ),
                );
            }
            Self::Score(question) => {
                if question.criteria.is_empty() {
                    return Err(Error::validation(format!(
                        "Score question \"{name}\" has no criteria; at least one score is required."
                    )));
                }
                map.insert("type".into(), Value::String("score".into()));
                insert_instructions(&mut map, question.instructions);
                map.insert(
                    "criteria".into(),
                    Value::Array(
                        question
                            .criteria
                            .into_iter()
                            .map(JsonContent::into_value)
                            .collect(),
                    ),
                );
            }
            Self::Raw(raw) => {
                validate_raw(name, &raw)?;
                return Ok(Value::Object(raw));
            }
        }
        Ok(Value::Object(map))
    }
}

/// An ordered collection of named System One questions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Questions {
    entries: IndexMap<String, Question>,
}

impl Questions {
    /// Creates an empty question collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty collection with capacity for `capacity` questions.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: IndexMap::with_capacity(capacity),
        }
    }

    /// Adds a named question, replacing an existing question with the same name.
    pub fn add(mut self, name: impl Into<String>, question: impl Into<Question>) -> Self {
        self.entries.insert(name.into(), question.into());
        self
    }

    /// Inserts a named question, returning the previous value when present.
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        question: impl Into<Question>,
    ) -> Option<Question> {
        self.entries.insert(name.into(), question.into())
    }

    /// Returns the number of questions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the collection has no questions.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn extend(&mut self, questions: Self) {
        self.entries.extend(questions.entries);
    }

    pub(crate) fn into_wire(self) -> Result<Map<String, Value>, Error> {
        if self.entries.is_empty() {
            return Err(Error::validation("At least one question is required."));
        }
        self.entries
            .into_iter()
            .map(|(name, question)| {
                let value = question.into_wire(&name)?;
                Ok((name, value))
            })
            .collect()
    }
}

fn insert_instructions(map: &mut Map<String, Value>, instructions: Option<JsonContent>) {
    if let Some(value) = instructions {
        map.insert("instructions".into(), value.into_value());
    }
}

fn validate_raw(name: &str, map: &Map<String, Value>) -> Result<(), Error> {
    let kind = match map.get("type") {
        Some(Value::String(kind)) if !kind.is_empty() => kind.as_str(),
        _ => {
            return Err(Error::validation(format!(
                "Question \"{name}\" must be a question object with a nonempty string \"type\"."
            )));
        }
    };
    if matches!(kind, "choice" | "score") && !map.contains_key("criteria") {
        return Err(Error::validation(format!(
            "Question \"{name}\" requires \"criteria\"."
        )));
    }
    if kind == "score"
        && matches!(map.get("criteria"), Some(Value::Array(items)) if items.is_empty())
    {
        return Err(Error::validation(format!(
            "Score question \"{name}\" has no criteria; at least one score is required."
        )));
    }
    Ok(())
}

impl From<Noul> for Question {
    fn from(value: Noul) -> Self {
        Self::Noul(value)
    }
}

impl From<Choice> for Question {
    fn from(value: Choice) -> Self {
        Self::Choice(value)
    }
}

impl From<Score> for Question {
    fn from(value: Score) -> Self {
        Self::Score(value)
    }
}

impl Serialize for Question {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.clone()
            .into_wire("question")
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn typed_questions_match_the_wire_contract() {
        let question: Question = Noul::new("Spam?")
            .criteria(NoulCriteria::new().yes_null().no("not spam"))
            .into();
        assert_eq!(
            question.into_wire("spam").unwrap(),
            json!({
                "type": "noul",
                "instructions": "Spam?",
                "criteria": {"true": null, "false": "not spam"}
            })
        );
    }

    #[test]
    fn raw_questions_are_minimally_validated() {
        let raw = json!({"type": "future", "weight": 2})
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(
            Question::raw(raw.clone()).into_wire("q").unwrap(),
            Value::Object(raw)
        );
        assert!(Question::raw(Map::new()).into_wire("q").is_err());
    }

    #[test]
    fn score_must_have_criteria() {
        let score: Question = Score::new("Quality?").into();
        assert!(score.into_wire("quality").is_err());
    }

    #[test]
    fn fluent_question_collection_preserves_order() {
        let questions = Questions::new()
            .add("billing", Noul::new("Billing?"))
            .add("tone", Choice::new("Tone?").options(["calm", "angry"]))
            .add("urgency", Score::new("Urgency?").levels(["low", "high"]));
        let wire = questions.into_wire().unwrap();
        assert_eq!(
            wire.keys().map(String::as_str).collect::<Vec<_>>(),
            ["billing", "tone", "urgency"]
        );
        assert_eq!(
            wire["tone"]["criteria"],
            json!({"calm": null, "angry": null})
        );
    }
}
