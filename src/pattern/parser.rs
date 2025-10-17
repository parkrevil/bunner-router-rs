use regex::Regex;

use crate::pattern::ast::{
    GroupNode, ParameterConstraint, ParameterNode, PatternAst, PatternNode, Quantifier,
    WildcardNode,
};
use crate::pattern::{PatternError, PatternResult};
use crate::router::ParamStyle;

pub fn parse_pattern(pattern: &str) -> PatternResult<PatternAst> {
    let mut parser = PatternParser::new(pattern);
    let nodes = parser.parse_sequence(None, None)?;
    if parser.peek().is_some() {
        // Should never happen due to parser exhausting input, but guard just in case.
        return Err(PatternError::UnexpectedClosingParenthesis {
            pattern: pattern.to_string(),
            index: parser.current_byte_index(),
        });
    }

    let ast = PatternAst::new(nodes);
    validate_ast(&ast, pattern)?;
    Ok(ast)
}

struct PatternParser<'a> {
    pattern: &'a str,
    chars: Vec<(usize, char)>,
    index: usize,
}

impl<'a> PatternParser<'a> {
    fn new(pattern: &'a str) -> Self {
        let chars: Vec<(usize, char)> = pattern.char_indices().collect();
        Self {
            pattern,
            chars,
            index: 0,
        }
    }

    fn parse_sequence(
        &mut self,
        terminator: Option<char>,
        group_start: Option<usize>,
    ) -> PatternResult<Vec<PatternNode>> {
        let mut nodes = Vec::new();
        while let Some(ch) = self.peek() {
            if Some(ch) == terminator {
                self.next();
                return Ok(nodes);
            }
            match ch {
                ')' => {
                    return Err(PatternError::UnexpectedClosingParenthesis {
                        pattern: self.pattern.to_string(),
                        index: self.current_byte_index(),
                    });
                }
                '?' | '+' => {
                    return Err(PatternError::DanglingQuantifier {
                        pattern: self.pattern.to_string(),
                        index: self.current_byte_index(),
                        modifier: ch,
                    });
                }
                ':' => {
                    nodes.push(self.parse_colon_parameter()?);
                }
                '{' => {
                    nodes.push(self.parse_braced_parameter()?);
                }
                '(' => {
                    nodes.push(self.parse_group()?);
                }
                '*' => {
                    nodes.push(self.parse_wildcard()?);
                }
                _ => {
                    nodes.push(self.parse_literal()?);
                }
            }
        }

        if terminator.is_some() {
            return Err(PatternError::UnterminatedGroup {
                pattern: self.pattern.to_string(),
                start: group_start.unwrap_or(self.pattern.len()),
            });
        }

        Ok(nodes)
    }

    fn parse_literal(&mut self) -> PatternResult<PatternNode> {
        let mut literal = String::new();
        while let Some(ch) = self.peek() {
            match ch {
                ':' | '{' | '(' | ')' | '*' => {
                    break;
                }
                _ => {
                    if self.is_escape_char(ch) {
                        literal.push(self.consume_escape_char()?);
                    } else {
                        literal.push(ch);
                        self.next();
                    }
                }
            }
        }
        Ok(PatternNode::Literal(literal))
    }

    fn parse_group(&mut self) -> PatternResult<PatternNode> {
        let start_index = self.current_byte_index();
        self.expect('(');
        let nodes = self.parse_sequence(Some(')'), Some(start_index))?;
        if nodes.is_empty() {
            return Err(PatternError::EmptyGroup {
                pattern: self.pattern.to_string(),
                start: start_index,
            });
        }
        let quantifier = self.parse_quantifier();
        Ok(PatternNode::Group(GroupNode::new(nodes, quantifier)))
    }

    fn parse_wildcard(&mut self) -> PatternResult<PatternNode> {
        let index = self.current_byte_index();
        self.expect('*');
        if let Some(modifier @ ('?' | '*' | '+')) = self.peek() {
            return Err(PatternError::WildcardQuantifierUnsupported {
                pattern: self.pattern.to_string(),
                index,
                modifier,
            });
        }
        Ok(PatternNode::Wildcard(WildcardNode::new(Quantifier::One)))
    }

    fn parse_colon_parameter(&mut self) -> PatternResult<PatternNode> {
        let name_start_byte = self.current_byte_index();
        self.expect(':');
        let mut name = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                name.push(ch);
                self.next();
            } else {
                break;
            }
        }

        if name.is_empty() {
            return Err(PatternError::ParameterMissingName {
                segment: self.pattern.to_string(),
            });
        }

        let bytes = name.as_bytes();
        if !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
            return Err(PatternError::ParameterInvalidStart {
                segment: self.pattern.to_string(),
                name: name.clone(),
                found: bytes[0] as char,
            });
        }

        for &c in &bytes[1..] {
            if !(c.is_ascii_alphanumeric() || c == b'_') {
                return Err(PatternError::ParameterInvalidCharacter {
                    segment: self.pattern.to_string(),
                    name: name.clone(),
                    invalid: c as char,
                });
            }
        }

        let constraint = if self.peek() == Some('(') {
            let constraint = self.parse_inline_constraint(name.clone(), name_start_byte)?;
            Some(ParameterConstraint::new(constraint))
        } else {
            None
        };

        let quantifier = self.parse_quantifier();

        Ok(PatternNode::Parameter(ParameterNode::new(
            name,
            constraint,
            quantifier,
            ParamStyle::Colon,
        )))
    }

    fn parse_braced_parameter(&mut self) -> PatternResult<PatternNode> {
        let brace_start = self.current_byte_index();
        self.expect('{');
        let mut name = String::new();
        while let Some(ch) = self.peek() {
            match ch {
                '}' | ':' => {
                    break;
                }
                _ => {
                    if self.is_escape_char(ch) {
                        name.push(self.consume_escape_char()?);
                    } else {
                        name.push(ch);
                        self.next();
                    }
                }
            }
        }

        if name.is_empty() {
            return Err(PatternError::ParameterMissingName {
                segment: self.pattern.to_string(),
            });
        }

        let bytes = name.as_bytes();
        if !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
            return Err(PatternError::ParameterInvalidStart {
                segment: self.pattern.to_string(),
                name: name.clone(),
                found: bytes[0] as char,
            });
        }
        for &c in &bytes[1..] {
            if !(c.is_ascii_alphanumeric() || c == b'_') {
                return Err(PatternError::ParameterInvalidCharacter {
                    segment: self.pattern.to_string(),
                    name: name.clone(),
                    invalid: c as char,
                });
            }
        }

        let constraint = if self.peek() == Some(':') {
            self.next();
            Some(ParameterConstraint::new(
                self.read_until_closing_brace(name.clone(), brace_start)?,
            ))
        } else {
            None
        };

        if self.peek() != Some('}') {
            return Err(PatternError::UnterminatedParameterConstraint {
                pattern: self.pattern.to_string(),
                name,
                start: brace_start,
            });
        }
        self.expect('}');

        let quantifier = self.parse_quantifier();
        Ok(PatternNode::Parameter(ParameterNode::new(
            name,
            constraint,
            quantifier,
            ParamStyle::Braces,
        )))
    }

    fn parse_inline_constraint(
        &mut self,
        name: String,
        start_byte: usize,
    ) -> PatternResult<String> {
        self.expect('(');
        let mut depth = 1usize;
        let mut constraint = String::new();
        while let Some(ch) = self.peek() {
            if self.is_escape_char(ch) {
                let escaped = self.consume_escape_char()?;
                constraint.push('\\');
                constraint.push(escaped);
                continue;
            }
            match ch {
                '(' => {
                    depth += 1;
                    constraint.push(ch);
                    self.next();
                }
                ')' => {
                    depth -= 1;
                    self.next();
                    if depth == 0 {
                        return Ok(constraint);
                    }
                    constraint.push(')');
                }
                _ => {
                    constraint.push(ch);
                    self.next();
                }
            }
        }

        Err(PatternError::UnterminatedParameterConstraint {
            pattern: self.pattern.to_string(),
            name,
            start: start_byte,
        })
    }

    fn read_until_closing_brace(
        &mut self,
        name: String,
        start_byte: usize,
    ) -> PatternResult<String> {
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch == '}' {
                break;
            }
            if self.is_escape_char(ch) {
                let escaped = self.consume_escape_char()?;
                value.push('\\');
                value.push(escaped);
            } else {
                value.push(ch);
                self.next();
            }
        }

        if self.peek() != Some('}') {
            return Err(PatternError::UnterminatedParameterConstraint {
                pattern: self.pattern.to_string(),
                name,
                start: start_byte,
            });
        }
        Ok(value)
    }

    fn parse_quantifier(&mut self) -> Quantifier {
        match self.peek() {
            Some('?') => {
                self.next();
                Quantifier::ZeroOrOne
            }
            Some('*') => {
                self.next();
                Quantifier::ZeroOrMore
            }
            Some('+') => {
                self.next();
                Quantifier::OneOrMore
            }
            _ => Quantifier::One,
        }
    }

    fn expect(&mut self, expected: char) {
        let actual = self.next();
        debug_assert_eq!(Some(expected), actual);
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).map(|(_, ch)| *ch)
    }

    fn next(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.index += 1;
        }
        ch
    }

    fn current_byte_index(&self) -> usize {
        self.chars
            .get(self.index)
            .map(|(idx, _)| *idx)
            .unwrap_or_else(|| self.pattern.len())
    }

    fn is_escape_char(&self, ch: char) -> bool {
        ch == '\\'
    }

    fn consume_escape_char(&mut self) -> PatternResult<char> {
        let escape_index = self.current_byte_index();
        debug_assert!(self.next().is_some());
        match self.next() {
            Some(ch) => Ok(ch),
            None => Err(PatternError::LoneEscapeCharacter {
                pattern: self.pattern.to_string(),
                index: escape_index,
            }),
        }
    }
}

fn validate_ast(ast: &PatternAst, pattern: &str) -> PatternResult<()> {
    validate_nodes(&ast.nodes, pattern, 0)?;
    validate_constraints(&ast.nodes, pattern)?;
    Ok(())
}

#[allow(clippy::only_used_in_recursion)]
fn validate_nodes(
    nodes: &[PatternNode],
    pattern: &str,
    optional_depth: usize,
) -> PatternResult<()> {
    for node in nodes {
        match node {
            PatternNode::Parameter(_param) => {}
            PatternNode::Group(group) => {
                let mut next_optional_depth = optional_depth;
                if group.quantifier.is_optional() {
                    next_optional_depth += 1;
                }

                validate_nodes(&group.nodes, pattern, next_optional_depth)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_constraints(
    nodes: &[PatternNode],
    pattern: &str,
) -> PatternResult<()> {
    for node in nodes {
        match node {
            PatternNode::Parameter(param) => {
                if let Some(constraint) = &param.constraint {
                    let source = format!("^(?:{})$", constraint.raw);
                    if let Err(err) = Regex::new(&source) {
                        return Err(PatternError::RegexConstraintInvalid {
                            pattern: pattern.to_string(),
                            name: param.name.clone(),
                            error: err.to_string(),
                        });
                    }
                }
            }
            PatternNode::Group(group) => {
                validate_constraints(&group.nodes, pattern)?;
            }
            _ => {}
        }
    }
    Ok(())
}
