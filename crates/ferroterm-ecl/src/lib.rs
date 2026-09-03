//! The Expression Constraint Language.
//!
//! A `logos` lexer and a `winnow` parser faithful to the official ECL ANTLR
//! grammar (`vendor/syntax/ECL.g4`, the pinned tag in `docs/VERSIONS.md`;
//! <https://docs.snomed.org/snomed-ct-specifications/snomed-ct-expression-constraint-language>),
//! a syntax tree named after the grammar's rules, and a printer whose output
//! parses back to the same tree.
#![doc(test(attr(deny(warnings))))]

pub mod ast;
pub mod dialects;
pub mod eval;
pub mod lexer;
pub mod parser;
mod print;

use winnow::prelude::*;
use winnow::stream::TokenSlice;

use crate::ast::ExpressionConstraint;
use crate::lexer::LexError;

/// A malformed expression constraint.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// A character no token starts with.
    #[error(transparent)]
    Lex(#[from] LexError),
    /// The tokens do not form an expression constraint.
    #[error("expected {expected} at byte {offset}, found {found}")]
    Syntax {
        /// The byte offset where the parser stopped.
        offset: usize,
        /// The token class the grammar admits there.
        expected: String,
        /// The text found there, or "the end of the expression".
        found: String,
    },
}

impl ParseError {
    /// The byte offset the error points at.
    #[must_use]
    pub fn offset(&self) -> usize {
        match self {
            Self::Lex(error) => error.offset,
            Self::Syntax { offset, .. } => *offset,
        }
    }
}

/// Parses an expression constraint.
///
/// # Errors
///
/// Returns [`ParseError`] with the byte offset of the first character or
/// token the grammar does not admit. An identifier that names no concept
/// still parses; the evaluator refuses it.
///
/// # Examples
///
/// ```
/// let tree = ferroterm_ecl::parse("<< 73211009 |Diabetes mellitus|")?;
/// assert_eq!(tree.to_string(), "<< 73211009 |Diabetes mellitus|");
/// # Ok::<(), ferroterm_ecl::ParseError>(())
/// ```
pub fn parse(input: &str) -> Result<ExpressionConstraint, ParseError> {
    let tokens = lexer::lex(input)?;
    parser::whole
        .parse(TokenSlice::new(&tokens))
        .map_err(|error| {
            let index = error.offset();
            let (offset, found) = tokens.get(index).map_or_else(
                || (input.len(), String::from("the end of the expression")),
                |token| (token.span.start, format!("{:?}", token.text)),
            );
            ParseError::Syntax {
                offset,
                expected: String::from(
                    error
                        .inner()
                        .expected
                        .unwrap_or("a valid expression constraint"),
                ),
                found,
            }
        })
}

impl std::str::FromStr for ExpressionConstraint {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse(s)
    }
}
