use crate::router::ParamStyle;

#[derive(Debug, Clone, PartialEq)]
pub struct PatternAst {
    pub nodes: Vec<PatternNode>,
}

impl PatternAst {
    pub fn new(nodes: Vec<PatternNode>) -> Self {
        Self { nodes }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatternNode {
    Literal(String),
    Parameter(ParameterNode),
    Group(GroupNode),
    Wildcard(WildcardNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterNode {
    pub name: String,
    pub constraint: Option<ParameterConstraint>,
    pub quantifier: Quantifier,
    pub style: ParamStyle,
}

impl ParameterNode {
    pub fn new(
        name: String,
        constraint: Option<ParameterConstraint>,
        quantifier: Quantifier,
        style: ParamStyle,
    ) -> Self {
        Self {
            name,
            constraint,
            quantifier,
            style,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterConstraint {
    pub raw: String,
}

impl ParameterConstraint {
    pub fn new(raw: String) -> Self {
        Self { raw }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupNode {
    pub nodes: Vec<PatternNode>,
    pub quantifier: Quantifier,
}

impl GroupNode {
    pub fn new(nodes: Vec<PatternNode>, quantifier: Quantifier) -> Self {
        Self { nodes, quantifier }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WildcardNode {
    pub quantifier: Quantifier,
}

impl WildcardNode {
    pub fn new(quantifier: Quantifier) -> Self {
        Self { quantifier }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    One,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
}

impl Quantifier {
    pub fn from_modifier(ch: Option<char>) -> Option<Self> {
        match ch {
            Some('?') => Some(Self::ZeroOrOne),
            Some('*') => Some(Self::ZeroOrMore),
            Some('+') => Some(Self::OneOrMore),
            _ => None,
        }
    }

    pub fn is_optional(&self) -> bool {
        matches!(self, Self::ZeroOrOne | Self::ZeroOrMore)
    }

    pub fn is_repeating(&self) -> bool {
        matches!(self, Self::ZeroOrMore | Self::OneOrMore)
    }
}
