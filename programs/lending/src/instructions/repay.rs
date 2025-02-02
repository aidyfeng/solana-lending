use std::f64::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::ErrorCode,
    state::{Bank, User},
};

#[derive(Accounts)]
pub struct Repay<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    pub repay_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [repay_mint.key().as_ref()],
        bump
    )]
    pub repay_bank: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [b"treasury",repay_mint.key().as_ref()],
        bump
    )]
    pub repay_bank_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [signer.key().as_ref()],
        bump
    )]
    pub user: Account<'info, User>,

    #[account(
        mut,
        associated_token::authority = signer,
        associated_token::mint = repay_mint,
        associated_token::token_program = token_program
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn process_repay(ctx: Context<Repay>, amount: u64) -> Result<()> {
    let user = &mut ctx.accounts.user;

    let borrow_value = if ctx.accounts.repay_mint.key() == user.usdc_address {
        user.borrowed_usdc
    } else {
        user.borrowed_sol
    };

    let time_diff = -user.last_updated_borrowed - Clock::get()?.unix_timestamp;

    let bank = &mut ctx.accounts.repay_bank;
    bank.total_borrowed =
        (bank.total_borrowed as f64 * (E.powf(bank.instrest_rate * time_diff as f64))) as u64;

    let value_per_share = bank.total_borrowed as f64 / bank.total_borrowed_shares as f64;

    let user_value = borrow_value as f64 / value_per_share;

    if amount as f64 > user_value {
        return Err(ErrorCode::OverRepay.into());
    }

    //transfer token
    let transfer_cpi_account = token_interface::TransferChecked {
        from: ctx.accounts.user_token_account.to_account_info(),
        mint: ctx.accounts.repay_mint.to_account_info(),
        to: ctx.accounts.repay_bank_token_account.to_account_info(),
        authority: ctx.accounts.repay_bank_token_account.to_account_info(),
    };

    let cip_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        transfer_cpi_account,
    );

    token_interface::transfer_checked(cip_ctx, amount, ctx.accounts.repay_mint.decimals)?;

    let borrow_radio = amount.checked_div(bank.total_borrowed).unwrap();
    let user_shares = bank
        .total_borrowed_shares
        .checked_mul(borrow_radio)
        .unwrap();

    if ctx.accounts.repay_mint.key() == user.usdc_address {
        user.borrowed_usdc -= amount;
        user.borrowed_usdc_shares -= user_shares;
    } else {
        user.deposited_sol -= amount;
        user.deposited_sol_shares -= user_shares;
    }

    bank.total_borrowed -= amount;
    bank.total_borrowed_shares -= user_shares;

    Ok(())
}
