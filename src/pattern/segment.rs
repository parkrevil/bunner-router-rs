use regex::Regex;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ParamConstraint {
    raw: Box<str>,
    compiled: Option<Arc<Regex>>,
}

impl ParamConstraint {
    pub fn new(raw: String) -> Self {
        Self {
            raw: raw.into_boxed_str(),
            compiled: None,
        }
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn compiled(&self) -> Option<&Arc<Regex>> {
        self.compiled.as_ref()
    }

    pub fn set_compiled(&mut self, regex: Arc<Regex>) {
        self.compiled = Some(regex);
    }
}

impl PartialEq for ParamConstraint {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for ParamConstraint {}

#[derive(Debug, Clone)]
pub enum SegmentPart {
    Literal(String),
    Param {
        name: String,
        constraint: Option<ParamConstraint>,
    },
}

#[derive(Debug, Clone)]
pub struct SegmentPattern {
    pub parts: Vec<SegmentPart>,
}

impl PartialEq for SegmentPattern {
    fn eq(&self, other: &Self) -> bool {
        if self.parts.len() != other.parts.len() {
            return false;
        }
        for (a, b) in self.parts.iter().zip(other.parts.iter()) {
            match (a, b) {
                (SegmentPart::Literal(la), SegmentPart::Literal(lb)) => {
                    if la != lb {
                        return false;
                    }
                }
                (
                    SegmentPart::Param {
                        name: na,
                        constraint: ca,
                    },
                    SegmentPart::Param {
                        name: nb,
                        constraint: cb,
                    },
                ) => {
                    if na != nb {
                        return false;
                    }
                    if ca.as_ref().map(|c| c.raw()) != cb.as_ref().map(|c| c.raw()) {
                        return false;
                    }
                }
                _ => {
                    return false;
                }
            }
        }
        true
    }
}
