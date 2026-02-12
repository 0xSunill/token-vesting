use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[cfg(test)]
mod tests;

declare_id!("H2CpMZemyu7b1R5AVPcYsqAdYaE1oJ8D2YwbrSGdZaLT");

#[program]
pub mod vesting {
    use super::*;

    pub fn create_vesting_account(
        ctx: Context<CreateVestingAccount>,
        company_name: String,
        amount: u64,
    ) -> Result<()> {
        *ctx.accounts.vesting_account = VestingAccount {
            owner: ctx.accounts.signer.key(),
            mint: ctx.accounts.mint.key(),
            treasury_account: ctx.accounts.treasury_token_account.key(),
            company_name: company_name,
            treasury_bump: ctx.bumps.treasury_token_account,
            bump: ctx.bumps.vesting_account,
        };
        Ok(())
    }

    pub fn create_employee_account(
        ctx: Context<CreateEmployeeAccount>,
        start_time: i64,
        end_time: i64,
        total_amount: u64,
        cliff_time: i64,
    ) -> Result<()> {
        *ctx.accounts.employee_account = EmployeeAccount {
            benificiary: ctx.accounts.beneficiary.key(),
            start_time,
            end_time,
            cliff_time,
            vesting_account: ctx.accounts.vesting_account.key(),
            total_amount,
            total_claimed: 0,
            bump: ctx.bumps.employee_account,
        };

        Ok(())
    }


#[derive(Accounts)]
#[instruction(company_name:String)]
pub struct CreateVestingAccount<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        init,
        payer = signer,
        space = 8 + VestingAccount::INIT_SPACE,
        seeds = [company_name.as_ref()],
        bump,
    )]
    pub vesting_account: Account<'info, VestingAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        token::mint = mint,
        token::authority = treasury_token_account,
        payer = signer,
        seeds = [b"vesting_treasury", company_name.as_bytes()],
        bump
    )]
    pub treasury_token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Program<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateEmployeeAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    pub beneficiary: SystemAccount<'info>,

    #[account(
        has_one = owner
    )]
    pub vesting_account: Account<'info, VestingAccount>,

    #[account(
    init,
    space = 8+ EmployeeAccount::INIT_SPACE,
    payer = owner,
    seeds = [b"employee_vesting", beneficiary.key().as_ref(),vesting_account.key().as_ref()],
    bump,
    )]
    pub employee_account: Account<'info, EmployeeAccount>,

    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct VestingAccount {
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub treasury_account: Pubkey,
    #[max_len(32)]
    pub company_name: String,
    pub treasury_bump: u8,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct EmployeeAccount {
    pub benificiary: Pubkey,
    pub start_time: i64,
    pub end_time: i64,
    pub cliff_time: i64, //time how much an employee has to wait before claming
    pub vesting_account: Pubkey,
    pub total_amount: u64,
    pub total_claimed: u64,
    pub bump: u8,
}
