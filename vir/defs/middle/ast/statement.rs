pub(crate) use super::{
    super::{cfg::procedure::BasicBlockId, operations_internal::ty::Typed, Expression, Position},
    predicate::Predicate,
    rvalue::{Operand, Rvalue},
    ty::{Type, VariantIndex},
    variable::VariableDecl,
};
use crate::common::display;

#[derive_helpers]
#[derive_visitors]
#[derive(derive_more::From, derive_more::IsVariant)]
#[allow(clippy::large_enum_variant)]
pub enum Statement {
    Comment(Comment),
    Inhale(Inhale),
    Exhale(Exhale),
    Consume(Consume),
    Assert(Assert),
    FoldOwned(FoldOwned),
    UnfoldOwned(UnfoldOwned),
    JoinBlock(JoinBlock),
    SplitBlock(SplitBlock),
    ConvertOwnedIntoMemoryBlock(ConvertOwnedIntoMemoryBlock),
    MovePlace(MovePlace),
    CopyPlace(CopyPlace),
    WritePlace(WritePlace),
    WriteAddress(WriteAddress),
    Assign(Assign),
    NewLft(NewLft),
    EndLft(EndLft),
    GhostAssignment(GhostAssignment),
    Borrow(Borrow),
}

#[display(fmt = "// {}", comment)]
pub struct Comment {
    pub comment: String,
}

/// Inhale the permission denoted by the place.
#[display(fmt = "inhale {}", predicate)]
pub struct Inhale {
    pub predicate: Predicate,
    pub position: Position,
}

#[display(fmt = "exhale {}", predicate)]
/// Exhale the permission denoted by the place.
pub struct Exhale {
    pub predicate: Predicate,
    pub position: Position,
}

#[display(fmt = "consume {}", operand)]
/// Consume the operand.
pub struct Consume {
    pub operand: Operand,
    pub position: Position,
}

#[display(fmt = "assert {}", expression)]
/// Assert the boolean expression.
pub struct Assert {
    pub expression: Expression,
    pub position: Position,
}

#[display(
    fmt = "fold{} {}",
    "display::option_foreach!(condition, \"<{}>\", \"{},\", \"\")",
    place
)]
/// Fold `OwnedNonAliased(place)`.
pub struct FoldOwned {
    pub place: Expression,
    pub condition: Option<Vec<BasicBlockId>>,
    pub position: Position,
}

#[display(
    fmt = "unfold{} {}",
    "display::option_foreach!(condition, \"<{}>\", \"{},\", \"\")",
    place
)]
/// Unfold `OwnedNonAliased(place)`.
pub struct UnfoldOwned {
    pub place: Expression,
    pub condition: Option<Vec<BasicBlockId>>,
    pub position: Position,
}

#[display(
    fmt = "join{} {}{}",
    "display::option_foreach!(condition, \"<{}>\", \"{},\", \"\")",
    place,
    "display::option!(enum_variant, \"[{}]\", \"\")"
)]
/// Join `MemoryBlock(place)`.
pub struct JoinBlock {
    pub place: Expression,
    pub condition: Option<Vec<BasicBlockId>>,
    /// If we are joining ex-enum, then we need to know for which variant.
    pub enum_variant: Option<VariantIndex>,
    pub position: Position,
}

#[display(
    fmt = "split{} {}{}",
    "display::option_foreach!(condition, \"<{}>\", \"{},\", \"\")",
    place,
    "display::option!(enum_variant, \"[{}]\", \"\")"
)]
/// Split `MemoryBlock(place)`.
pub struct SplitBlock {
    pub place: Expression,
    pub condition: Option<Vec<BasicBlockId>>,
    /// If we are splitting for enum, then we need to know for which variant.
    pub enum_variant: Option<VariantIndex>,
    pub position: Position,
}

/// Convert `Owned(place)` into `MemoryBlock(place)`.
#[display(
    fmt = "convert-owned-memory-block{} {}",
    "display::option_foreach!(condition, \"<{}>\", \"{},\", \"\")",
    place
)]
pub struct ConvertOwnedIntoMemoryBlock {
    pub place: Expression,
    pub condition: Option<Vec<BasicBlockId>>,
    pub position: Position,
}

#[display(fmt = "move {} ← {}", target, source)]
pub struct MovePlace {
    pub target: Expression,
    pub source: Expression,
    pub position: Position,
}

#[display(fmt = "copy {} ← {}", target, source)]
/// Copy assignment.
pub struct CopyPlace {
    pub target: Expression,
    pub source: Expression,
    pub position: Position,
}

#[display(fmt = "write_place {} := {}", target, value)]
pub struct WritePlace {
    /// A place to write the value into.
    pub target: Expression,
    pub value: Expression,
    pub position: Position,
}

#[display(fmt = "write_address {} := {}", target, value)]
pub struct WriteAddress {
    /// An adddress to write the value into.
    pub target: Expression,
    pub value: Expression,
    pub position: Position,
}

#[display(fmt = "assign {} := {}", target, value)]
pub struct Assign {
    pub target: Expression,
    pub value: Rvalue,
    pub position: Position,
}

#[display(fmt = "{} = newlft()", name)]
pub struct NewLft {
    pub name: String,
    pub position: Position,
}

#[display(fmt = "endlft({})", name)]
pub struct EndLft {
    pub name: String,
    pub position: Position,
}

#[display(fmt = "ghost-assign {} := {}", target, value)]
pub struct GhostAssignment {
    pub target: VariableDecl,
    pub value: Expression,
    pub position: Position,
}

#[display(fmt = "borrow({}, 1/{}, {})", lifetime, rd_perm, reference)]
pub struct Borrow {
    pub lifetime: String,
    pub rd_perm: u32,
    pub reference: Expression,
    pub position: Position,
}
