use regex::escape;

use super::ast::{GroupNode, ParameterNode, PatternAst, PatternNode, Quantifier};
use super::{PatternResult, compile_pattern_ast, parse_pattern};
use crate::router::{ParamStyle, RepeatMatchMode};

#[derive(Debug, Clone, PartialEq)]
pub struct PatternAnalysis {
    pub pattern: String,
    pub ast: PatternAst,
    pub compiled: super::CompiledPattern,
    pub tokens: Vec<PatternToken>,
    pub regex: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatternToken {
    Literal {
        value: String,
    },
    Parameter {
        name: String,
        constraint: Option<String>,
        quantifier: Quantifier,
        style: ParamStyle,
    },
    Wildcard {
        quantifier: Quantifier,
    },
    GroupStart {
        quantifier: Quantifier,
    },
    GroupEnd,
}

pub fn compile(
    pattern: &str,
    repeat_mode: RepeatMatchMode,
) -> PatternResult<super::CompiledPattern> {
    let ast = parse_pattern(pattern)?;
    compile_pattern_ast(&ast, repeat_mode)
}

pub fn tokens(pattern: &str) -> PatternResult<Vec<PatternToken>> {
    let ast = parse_pattern(pattern)?;
    Ok(collect_tokens(&ast))
}

pub fn to_regex(
    pattern: &str,
    repeat_mode: RepeatMatchMode,
    default_param_pattern: &str,
) -> PatternResult<String> {
    let ast = parse_pattern(pattern)?;
    Ok(build_regex(&ast.nodes, repeat_mode, default_param_pattern))
}

pub fn analyze(
    pattern: &str,
    repeat_mode: RepeatMatchMode,
    default_param_pattern: &str,
) -> PatternResult<PatternAnalysis> {
    let ast = parse_pattern(pattern)?;
    let compiled = compile_pattern_ast(&ast, repeat_mode)?;
    let tokens = collect_tokens(&ast);
    let regex = build_regex(&ast.nodes, repeat_mode, default_param_pattern);

    Ok(PatternAnalysis {
        pattern: pattern.to_string(),
        ast,
        compiled,
        tokens,
        regex,
    })
}

fn collect_tokens(ast: &PatternAst) -> Vec<PatternToken> {
    let mut out = Vec::new();
    for node in &ast.nodes {
        collect_node_tokens(node, &mut out);
    }
    out
}

fn collect_node_tokens(node: &PatternNode, out: &mut Vec<PatternToken>) {
    match node {
        PatternNode::Literal(value) => out.push(PatternToken::Literal {
            value: value.clone(),
        }),
        PatternNode::Parameter(param) => out.push(PatternToken::Parameter {
            name: param.name.clone(),
            constraint: param
                .constraint
                .as_ref()
                .map(|constraint| constraint.raw.clone()),
            quantifier: param.quantifier,
            style: param.style,
        }),
        PatternNode::Group(group) => {
            out.push(PatternToken::GroupStart {
                quantifier: group.quantifier,
            });
            for inner in &group.nodes {
                collect_node_tokens(inner, out);
            }
            out.push(PatternToken::GroupEnd);
        }
        PatternNode::Wildcard(wild) => out.push(PatternToken::Wildcard {
            quantifier: wild.quantifier,
        }),
    }
}

fn build_regex(
    nodes: &[PatternNode],
    repeat_mode: RepeatMatchMode,
    default_param_pattern: &str,
) -> String {
    let mut regex = String::from("^");
    regex.push_str(&nodes_to_regex(nodes, repeat_mode, default_param_pattern));
    regex.push('$');
    regex
}

fn nodes_to_regex(
    nodes: &[PatternNode],
    repeat_mode: RepeatMatchMode,
    default_param_pattern: &str,
) -> String {
    let mut out = String::new();
    for node in nodes {
        out.push_str(&node_to_regex(node, repeat_mode, default_param_pattern));
    }
    out
}

fn node_to_regex(
    node: &PatternNode,
    repeat_mode: RepeatMatchMode,
    default_param_pattern: &str,
) -> String {
    match node {
        PatternNode::Literal(value) => escape(value),
        PatternNode::Parameter(param) => {
            parameter_to_regex(param, repeat_mode, default_param_pattern)
        }
        PatternNode::Wildcard(_wild) => String::from(".*"),
        PatternNode::Group(group) => group_to_regex(group, repeat_mode, default_param_pattern),
    }
}

fn parameter_to_regex(
    param: &ParameterNode,
    repeat_mode: RepeatMatchMode,
    default_param_pattern: &str,
) -> String {
    let body = param
        .constraint
        .as_ref()
        .map(|constraint| constraint.raw.as_str())
        .unwrap_or(default_param_pattern);
    let quant = quantifier_suffix(param.quantifier, repeat_mode);
    format!("(?:{}){}", body, quant)
}

fn group_to_regex(
    group: &GroupNode,
    repeat_mode: RepeatMatchMode,
    default_param_pattern: &str,
) -> String {
    let inner = nodes_to_regex(&group.nodes, repeat_mode, default_param_pattern);
    let quant = quantifier_suffix(group.quantifier, repeat_mode);
    format!("(?:{}){}", inner, quant)
}

fn quantifier_suffix(quantifier: Quantifier, repeat_mode: RepeatMatchMode) -> String {
    match quantifier {
        Quantifier::One => String::new(),
        Quantifier::ZeroOrOne => String::from("?"),
        Quantifier::ZeroOrMore => repeat_modifier("*", repeat_mode),
        Quantifier::OneOrMore => repeat_modifier("+", repeat_mode),
    }
}

fn repeat_modifier(base: &str, repeat_mode: RepeatMatchMode) -> String {
    match repeat_mode {
        RepeatMatchMode::Greedy => base.to_string(),
        RepeatMatchMode::Lazy => format!("{}?", base),
    }
}
