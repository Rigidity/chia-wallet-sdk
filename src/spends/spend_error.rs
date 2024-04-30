use clvm_traits::{FromClvmError, ToClvmError};
use clvmr::reduction::EvalErr;
use thiserror::Error;

/// Errors that can occur when spending a coin.
#[derive(Debug, Error)]
pub enum SpendError {
    /// An error occurred while converting to clvm.
    #[error("to clvm error: {0}")]
    ToClvm(#[from] ToClvmError),

    /// An error occurred while converting from clvm.
    #[error("from clvm error: {0}")]
    FromClvm(#[from] FromClvmError),

    /// An error occurred while evaluating a program.
    #[error("eval error: {0}")]
    Eval(#[from] EvalErr),
}
