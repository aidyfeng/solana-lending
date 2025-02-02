use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient Funds")]
    InsufficientFunds,
    #[msg("Over Borrowable Amount")]
    OverBorrowableAmount,
    #[msg("Over Repay")]
    OverRepay,
    #[msg("User is not Under collateralized,can't be liquidated")]
    NotUnderCollateralized,
}
