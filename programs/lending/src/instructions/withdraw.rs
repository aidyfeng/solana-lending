use std::ops::DerefMut;

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
pub struct Withdraw<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [mint.key().as_ref()],
        bump
    )]
    pub bank: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [b"treasury",mint.key().as_ref()],
        bump
    )]
    pub bank_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [signer.key().as_ref()],
        bump
    )]
    pub user: Account<'info, User>,

    #[account(
        mut,
        associated_token::authority = signer,
        associated_token::mint = mint,
        associated_token::token_program = token_program
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,

    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn process_withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    let user = &mut ctx.accounts.user;

    let deposited_value = if ctx.accounts.mint.key() == user.usdc_address {
        user.deposited_usdc
    } else {
        user.deposited_sol
    };

    if amount > deposited_value {
        return Err(ErrorCode::InsufficientFunds.into());
    }

    let transfer_cpi_account = token_interface::TransferChecked {
        from: ctx.accounts.bank.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.user.to_account_info(),
        authority: ctx.accounts.bank_token_account.to_account_info(),
    };

    let mint_key = ctx.accounts.mint.key();
    let signer_seeds: &[&[&[u8]]] = &[&[
        b"treasury",
        mint_key.as_ref(),
        &[ctx.bumps.bank_token_account],
    ]];

    let cip_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_cpi_account,
        signer_seeds,
    );

    token_interface::transfer_checked(cip_ctx, amount, ctx.accounts.mint.decimals)?;

    let bank = ctx.accounts.bank.deref_mut();
    let shares_to_remove = (amount / bank.total_deposits) * bank.total_deposit_shares;

    let user = ctx.accounts.user.deref_mut();

    if ctx.accounts.mint.key() == user.usdc_address {
        user.deposited_usdc -= amount;
        user.deposited_usdc_shares -= shares_to_remove;
    } else {
        user.deposited_sol -= amount;
        user.deposited_sol_shares -= shares_to_remove;
    }

    bank.total_deposits -= amount;
    bank.total_deposit_shares -= shares_to_remove;
    Ok(())
}
