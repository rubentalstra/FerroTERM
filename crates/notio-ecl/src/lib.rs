//! The Expression Constraint Language.
//!
//! A `logos` lexer and a `winnow` parser faithful to the official ECL ANTLR
//! grammar, plus an evaluator that compiles an expression constraint to set
//! algebra over the materialized graph. Diagnostics carry source spans.
#![doc(test(attr(deny(warnings))))]
