use crate::span::SourceSpan;
use serde::{Deserialize, Serialize};

/// Category of a mutation operator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OperatorCategory {
    /// Condition / comparison changes (for example `==` ↔ `!=`, `<` ↔ `>=`).
    Condition,

    /// Constant and boundary changes (for example `0` → `1`, `n` → `n±1`).
    Constant,

    /// Boolean connectives (for example inserting or removing `!`).
    BooleanConnective,

    /// Arithmetic expression changes (for example `+` ↔ `-`).
    Arithmetic,
}

/// Identifier for a specific mutation operator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutationOperator {
    /// Category this operator belongs to.
    pub category: OperatorCategory,

    /// Short, stable identifier for the operator (for example `eq_to_neq`).
    pub name: String,
}

/// Outcome of running the test suite against a single mutant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MutantOutcome {
    /// Mutant has not been executed.
    NotRun,

    /// Tests failed because of this mutant.
    Killed,

    /// Tests still passed with this mutant.
    Survived,

    /// Mutant could not be built or executed.
    Invalid,
}

/// Representation of a single first-order mutant at the Noir source level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mutant {
    /// Monotonically increasing identifier.
    pub id: u64,

    /// Operator applied to create this mutant.
    pub operator: MutationOperator,

    /// Location of the mutated snippet in the source code.
    pub span: SourceSpan,

    /// Original source snippet (before mutation).
    pub original_snippet: String,

    /// Mutated source snippet (after mutation).
    pub mutated_snippet: String,

    /// Outcome of running the test suite against this mutant.
    pub outcome: MutantOutcome,

    /// Duration of the test run for this mutant in milliseconds.
    ///
    /// `None` means the mutant has not been executed.
    pub duration_ms: Option<u64>,
}
