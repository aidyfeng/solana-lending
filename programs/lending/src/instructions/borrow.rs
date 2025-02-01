use std::f64::consts::E;

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface},
};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use crate::{
    error::ErrorCode,
    constants,
    state::{Bank, User},
};

#[derive(Accounts)]
pub struct Borrow<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    pub borrow_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [borrow_mint.key().as_ref()],
        bump
    )]
    pub borrow_bank: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [b"treasury",borrow_mint.key().as_ref()],
        bump
    )]
    pub borrow_bank_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [signer.key().as_ref()],
        bump
    )]
    pub user: Account<'info, User>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::authority = signer,
        associated_token::mint = borrow_mint,
        associated_token::token_program = token_program
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,

    pub price_update: Account<'info, PriceUpdateV2>,
}

pub fn process_borrow(ctx: Context<Borrow>, amount: u64) -> Result<()> {
    let borrow_bank = &mut ctx.accounts.borrow_bank;
    let user = &mut ctx.accounts.user;

    let price_update = &mut ctx.accounts.price_update;

    let total_collateral = match ctx.accounts.borrow_mint.key() {
        key if key == user.usdc_address => {
            let sol_fee_id = get_feed_id_from_hex(constants::SOL_USD_FEED_ID)?;
            let sol_price = price_update.get_price_no_older_than(&Clock::get()?, constants::MAX_AGE, &sol_fee_id)?;
            let new_value = caculate_accrued_interest(user.deposited_sol, borrow_bank.instrest_rate, user.last_updated)?;
            sol_price.price as u64 * new_value
        }
        _ => {
            let usdc_fee_id = get_feed_id_from_hex(constants::USDC_USD_FEED_ID)?;
            let usdc_price = price_update.get_price_no_older_than(&Clock::get()?, constants::MAX_AGE, &usdc_fee_id)?;
            let new_value = caculate_accrued_interest(user.deposited_usdc, borrow_bank.instrest_rate, user.last_updated)?;
            usdc_price.price as u64 * new_value
        },
    };

    let borrowable_amount = total_collateral.checked_mul(borrow_bank.liquidation_threshold).unwrap();

    if borrowable_amount < amount {
        return Err(ErrorCode::OverBorrowableAmount.into())
    }

    msg!("transfer from borrow_bank_token_account to user_token_account");
    let transfer_cpi_account = token_interface::TransferChecked {
        from: borrow_bank.to_account_info(),
        mint: ctx.accounts.borrow_mint.to_account_info(),
        to: ctx.accounts.user_token_account.to_account_info(),
        authority: borrow_bank.to_account_info(),
    };

    let mint_key = ctx.accounts.borrow_mint.key();
    let signer_seeds: &[&[&[u8]]] = &[&[
        b"treasury",
        mint_key.as_ref(),
        &[ctx.bumps.borrow_bank_token_account],
    ]];

    let cip_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_cpi_account,
        signer_seeds,
    );

    token_interface::transfer_checked(cip_ctx, amount, ctx.accounts.borrow_mint.decimals)?;

    if borrow_bank.total_borrowed == 0{
        borrow_bank.total_borrowed = amount;
        borrow_bank.total_borrowed_shares = amount;
    }

    let borrow_radio = amount.checked_div(borrow_bank.total_borrowed).unwrap();
    let user_shares = borrow_bank.total_borrowed_shares.checked_mul(borrow_radio).unwrap();

    match ctx.accounts.borrow_mint.key() {
        key if key == ctx.accounts.borrow_mint.key() => {
            user.borrowed_usdc += amount;
            user.borrowed_usdc_shares += user_shares;
        },
        _ => {
            user.borrowed_sol += amount;
            user.borrowed_sol_shares += user_shares;
        }
    }


    Ok(())
}

fn caculate_accrued_interest(deposited: u64, interest_rate: f64, last_updated: i64) -> Result<u64> {
    let current_time = Clock::get()?.unix_timestamp;
    let time_diff = current_time - last_updated;

    let new_value = (deposited as f64 * E.powf(interest_rate * time_diff as f64)) as u64;
    Ok(new_value)
}
