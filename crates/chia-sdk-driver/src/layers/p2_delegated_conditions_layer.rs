use chia_bls::PublicKey;
use chia_sdk_types::{
    P2DelegatedConditionsArgs, P2DelegatedConditionsSolution, P2_DELEGATED_CONDITIONS_PUZZLE_HASH,
};
use clvm_traits::FromClvm;
use clvm_utils::CurriedProgram;
use clvmr::{Allocator, NodePtr};

use crate::{DriverError, Layer, Puzzle, SpendContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P2DelegatedConditionsLayer {
    pub public_key: PublicKey,
}

impl Layer for P2DelegatedConditionsLayer {
    type Solution = P2DelegatedConditionsSolution;

    fn construct_puzzle(&self, ctx: &mut SpendContext) -> Result<NodePtr, DriverError> {
        let curried = CurriedProgram {
            program: ctx.p2_delegated_conditions_puzzle()?,
            args: P2DelegatedConditionsArgs::new(self.public_key),
        };
        ctx.alloc(&curried)
    }

    fn construct_solution(
        &self,
        ctx: &mut SpendContext,
        solution: Self::Solution,
    ) -> Result<NodePtr, DriverError> {
        ctx.alloc(&solution)
    }

    fn parse_puzzle(allocator: &Allocator, puzzle: Puzzle) -> Result<Option<Self>, DriverError> {
        let Some(puzzle) = puzzle.as_curried() else {
            return Ok(None);
        };

        if puzzle.mod_hash != P2_DELEGATED_CONDITIONS_PUZZLE_HASH {
            return Ok(None);
        }

        let args = P2DelegatedConditionsArgs::from_clvm(allocator, puzzle.args)?;

        Ok(Some(Self {
            public_key: args.public_key,
        }))
    }

    fn parse_solution(
        allocator: &Allocator,
        solution: NodePtr,
    ) -> Result<Self::Solution, DriverError> {
        Ok(P2DelegatedConditionsSolution::from_clvm(
            allocator, solution,
        )?)
    }
}