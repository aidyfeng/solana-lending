use std::f64::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::state::{Bank, User};

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

    let time_diff =  - user.last_updated_borrowed -  Clock::get()?.unix_timestamp;

    let bank = &mut ctx.accounts.repay_bank;
    bank.total_borrowed = (bank.total_borrowed as f64 * (E.powf(bank.instrest_rate * time_diff as f64))) as u64;

    todo!()
}
