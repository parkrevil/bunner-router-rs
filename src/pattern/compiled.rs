use super::PatternResult;
use super::ast::{GroupNode, ParameterNode, PatternAst, PatternNode, Quantifier};
use crate::router::{ParamStyle, RepeatMatchMode};

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPattern {
    pub elements: Vec<RouteElement>,
    pub has_wildcard: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RouteElement {
    Segment(SegmentElement),
    Group(GroupElement),
    Wildcard(WildcardElement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SegmentElement {
    pub atoms: Vec<SegmentAtom>,
}

impl SegmentElement {
    fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    fn push_literal(&mut self, value: String) {
        if value.is_empty() {
            return;
        }
        if let Some(SegmentAtom::Literal(existing)) = self.atoms.last_mut() {
            existing.push_str(&value);
        } else {
            self.atoms.push(SegmentAtom::Literal(value));
        }
    }

    fn push_parameter(&mut self, parameter: ParameterElement) {
        self.atoms.push(SegmentAtom::Parameter(parameter));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SegmentAtom {
    Literal(String),
    Parameter(ParameterElement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterElement {
    pub name: String,
    pub constraint: Option<String>,
    pub style: ParamStyle,
    pub quantifier: QuantifierSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupElement {
    pub elements: Vec<RouteElement>,
    pub quantifier: QuantifierSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WildcardElement {
    pub quantifier: QuantifierSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantifierSpan {
    pub min: u16,
    pub max: Option<u16>,
    pub greedy: bool,
}

impl QuantifierSpan {
    fn from_quantifier(quantifier: Quantifier, repeat_mode: RepeatMatchMode) -> Self {
        match quantifier {
            Quantifier::One => QuantifierSpan {
                min: 1,
                max: Some(1),
                greedy: true,
            },
            Quantifier::ZeroOrOne => QuantifierSpan {
                min: 0,
                max: Some(1),
                greedy: true,
            },
            Quantifier::ZeroOrMore => QuantifierSpan {
                min: 0,
                max: None,
                greedy: repeat_mode == RepeatMatchMode::Greedy,
            },
            Quantifier::OneOrMore => QuantifierSpan {
                min: 1,
                max: None,
                greedy: repeat_mode == RepeatMatchMode::Greedy,
            },
        }
    }
}

pub fn compile_pattern_ast(
    ast: &PatternAst,
    repeat_mode: RepeatMatchMode,
) -> PatternResult<CompiledPattern> {
    let mut has_wildcard = false;
    let elements = compile_sequence(&ast.nodes, repeat_mode, &mut has_wildcard)?;
    Ok(CompiledPattern {
        elements,
        has_wildcard,
    })
}

fn compile_sequence(
    nodes: &[PatternNode],
    repeat_mode: RepeatMatchMode,
    has_wildcard: &mut bool,
) -> PatternResult<Vec<RouteElement>> {
    let mut elements = Vec::new();
    let mut current_segment = SegmentElement { atoms: Vec::new() };

    for node in nodes {
        match node {
            PatternNode::Literal(value) => {
                current_segment.push_literal(value.clone());
            }
            PatternNode::Parameter(param) => {
                let parameter = compile_parameter(param, repeat_mode);
                current_segment.push_parameter(parameter);
            }
            PatternNode::Group(group) => {
                flush_segment(&mut current_segment, &mut elements);
                let compiled = compile_group(group, repeat_mode, has_wildcard)?;
                elements.push(RouteElement::Group(compiled));
            }
            PatternNode::Wildcard(wild) => {
                flush_segment(&mut current_segment, &mut elements);
                *has_wildcard = true;
                let quantifier = QuantifierSpan::from_quantifier(wild.quantifier, repeat_mode);
                elements.push(RouteElement::Wildcard(WildcardElement { quantifier }));
            }
        }
    }

    flush_segment(&mut current_segment, &mut elements);

    Ok(elements)
}

fn flush_segment(segment: &mut SegmentElement, elements: &mut Vec<RouteElement>) {
    if segment.is_empty() {
        return;
    }
    let mut new_segment = SegmentElement {
        atoms: Vec::with_capacity(segment.atoms.len()),
    };
    std::mem::swap(&mut new_segment.atoms, &mut segment.atoms);
    elements.push(RouteElement::Segment(new_segment));
}

fn compile_parameter(param: &ParameterNode, repeat_mode: RepeatMatchMode) -> ParameterElement {
    ParameterElement {
        name: param.name.clone(),
        constraint: param
            .constraint
            .as_ref()
            .map(|constraint| constraint.raw.clone()),
        style: param.style,
        quantifier: QuantifierSpan::from_quantifier(param.quantifier, repeat_mode),
    }
}

fn compile_group(
    group: &GroupNode,
    repeat_mode: RepeatMatchMode,
    has_wildcard: &mut bool,
) -> PatternResult<GroupElement> {
    let elements = compile_sequence(&group.nodes, repeat_mode, has_wildcard)?;
    let quantifier = QuantifierSpan::from_quantifier(group.quantifier, repeat_mode);
    Ok(GroupElement {
        elements,
        quantifier,
    })
}
