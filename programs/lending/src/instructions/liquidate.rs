use std::f64::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface},
};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use crate::{
    constants::{self, SOL_USD_FEED_ID, USDC_USD_FEED_ID},
    error::ErrorCode,
    state::{Bank, User},
};

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub liquidator: Signer<'info>,

    pub price_update: Account<'info, PriceUpdateV2>,

    pub collateral_mint: InterfaceAccount<'info, Mint>,

    pub borrowed_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [collateral_mint.key().as_ref()],
        bump
    )]
    pub collateral_bank: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [borrowed_mint.key().as_ref()],
        bump
    )]
    pub borrowed_bank: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [b"treasury",collateral_mint.key().as_ref()],
        bump
    )]
    pub collateral_bank_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"treasury",borrowed_mint.key().as_ref()],
        bump
    )]
    pub borrowed_bank_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [liquidator.key().as_ref()],
        bump
    )]
    pub user_account: Account<'info, User>,

    #[account(
        init_if_needed,
        payer = liquidator,
        associated_token::authority = liquidator,
        associated_token::mint = collateral_mint,
        associated_token::token_program = token_program
    )]
    pub liquidator_collateral_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = liquidator,
        associated_token::authority = liquidator,
        associated_token::mint = borrowed_mint,
        associated_token::token_program = token_program
    )]
    pub liquidator_borrowed_token_account: InterfaceAccount<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn process_liquidate(ctx: Context<Liquidate>) -> Result<()> {
    let collateral_bank = &mut ctx.accounts.collateral_bank;
    let borrowed_bank = &mut ctx.accounts.borrowed_bank;

    let user = &mut ctx.accounts.user_account;

    let sol_fee_id = get_feed_id_from_hex(SOL_USD_FEED_ID)?;
    let usdc_fee_id = get_feed_id_from_hex(USDC_USD_FEED_ID)?;

    let price_update = &mut ctx.accounts.price_update;
    let sol_price =
        price_update.get_price_no_older_than(&Clock::get()?, constants::MAX_AGE, &sol_fee_id)?;
    let usdc_price =
        price_update.get_price_no_older_than(&Clock::get()?, constants::MAX_AGE, &usdc_fee_id)?;

    let total_collateral: u64;
    let total_borrowed: u64;

    match ctx.accounts.collateral_mint.key() {
        key if key == user.usdc_address => {
            let new_usdc = caculate_accrued_interest(
                user.deposited_usdc,
                collateral_bank.instrest_rate,
                user.last_updated,
            )?;
            total_collateral = new_usdc * usdc_price.price as u64;
            let new_sol = caculate_accrued_interest(
                user.borrowed_sol,
                borrowed_bank.instrest_rate,
                user.last_updated_borrowed,
            )?;
            total_borrowed = new_sol * sol_price.price as u64;
        }
        _ => {
            let new_sol = caculate_accrued_interest(
                user.deposited_sol,
                collateral_bank.instrest_rate,
                user.last_updated,
            )?;
            total_collateral = new_sol * sol_price.price as u64;
            let new_usdc = caculate_accrued_interest(
                user.borrowed_usdc,
                borrowed_bank.instrest_rate,
                user.last_updated_borrowed,
            )?;
            total_borrowed = new_usdc * usdc_price.price as u64;
        }
    }

    let health_factor = total_collateral as f64 * collateral_bank.liquidation_threshold as f64
        / total_borrowed as f64;

    if health_factor >= 1.0 {
        return Err(ErrorCode::NotUnderCollateralized.into());
    }

    msg!("transfer from liquidator borrowed token Account to borrowed bank Account");
    let transfer_to_bank = token_interface::TransferChecked {
        from: ctx.accounts.liquidator_borrowed_token_account.to_account_info(),
        mint: ctx.accounts.borrowed_mint.to_account_info(),
        to: ctx.accounts.borrowed_bank_token_account.to_account_info(),
        authority: ctx.accounts.liquidator.to_account_info(),
    };

    let cpi_program = ctx.accounts.token_program.to_account_info();

    let cpi_context = CpiContext::new(cpi_program.clone(), transfer_to_bank);

    let liquidation_amount = total_borrowed.checked_mul(borrowed_bank.liquidation_close_factor).unwrap();

    token_interface::transfer_checked(cpi_context, liquidation_amount, ctx.accounts.borrowed_mint.decimals)?;


    msg!("transfer from liquidator borrowed token Account to borrowed bank Account");
    let liquidator_amount = liquidation_amount * collateral_bank.liquidation_bonus + liquidation_amount;

    let transfer_to_liquidator = token_interface::TransferChecked{
        from: ctx.accounts.collateral_bank_token_account.to_account_info(),
        mint: ctx.accounts.collateral_mint.to_account_info(),
        to: ctx.accounts.liquidator_collateral_token_account.to_account_info(),
        authority: ctx.accounts.collateral_bank_token_account.to_account_info(),
    };

    let mint_key = ctx.accounts.collateral_mint.key();
    let signer_seeds : &[&[&[u8]]] = &[&[
        b"treasury",
        mint_key.as_ref(),
        &[ctx.bumps.collateral_bank_token_account]
    ]];

    let cpi_ctx_to_liquidator = CpiContext::new_with_signer(cpi_program.clone(), transfer_to_liquidator, signer_seeds);

    token_interface::transfer_checked(cpi_ctx_to_liquidator, liquidator_amount, ctx.accounts.collateral_mint.decimals)?;


    Ok(())
}

fn caculate_accrued_interest(deposited: u64, interest_rate: f64, last_updated: i64) -> Result<u64> {
    let current_time = Clock::get()?.unix_timestamp;
    let time_diff = current_time - last_updated;

    let new_value = (deposited as f64 * E.powf(interest_rate * time_diff as f64)) as u64;
    Ok(new_value)
}
