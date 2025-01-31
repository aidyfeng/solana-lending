use anchor_lang::prelude::*;
use instructions::*;

mod instructions;
mod state;

declare_id!("9XCHC5dVRNSkZvmMNj9F9ZQXPfXYjD6BQH2trTtkqBs5");

#[program]
pub mod lending {

    use super::*;

    pub fn init_bank(
        ctx: Context<InitBank>,
        liquidation_threshold: u64,
        max_ltv: u64,
    ) -> Result<()> {
        instructions::process_init_bank(ctx, liquidation_threshold, max_ltv)
    }

    pub fn init_user(ctx: Context<InitUser>, usdc_address: Pubkey) -> Result<()> {
        instructions::process_init_user(ctx, usdc_address)
    }
}
