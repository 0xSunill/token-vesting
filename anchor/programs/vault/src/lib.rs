use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::associated_token::AssociatedToken;
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
            beneficiary: ctx.accounts.beneficiary.key(),
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

    pub fn claim_tokens(ctx: Context<ClaimTokens>, company_name: String) -> Result<()> {
        let employee_account = &mut ctx.accounts.employee_account;
        let current_time = Clock::get()?.unix_timestamp;

        if current_time < employee_account.cliff_time {
            return Err(ErrorCode::ClaimNotAvailable.into());
        }   

        // if current_time > employee_account.end_time {
        //     return Err(ErrorCode::VestingPeriodEnded.into());
        // }

        let time_since_start = current_time.saturating_sub(employee_account.start_time);
        let total_vesting_period = employee_account.end_time.saturating_sub(employee_account.start_time);

        if total_vesting_period == 0 {
            return Err(ErrorCode::InvalidVestingPeriod.into());
        }

        let vested_amount = if current_time >= employee_account.end_time {
            employee_account.total_amount
        } else {
            match employee_account.total_amount.checked_mul(time_since_start as u64) {
                Some(amount) => amount / total_vesting_period as u64,
                None => {
                    return Err(ErrorCode::OverflowError.into());
                }
            }
        }

        let amount_to_claim = vested_amount.saturating_sub(employee_account.total_claimed);

        if amount_to_claim == 0 {
            return Err(ErrorCode::NoTokensToClaim.into());
        }
        
        Ok(())
    }
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
        bump
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
    pub token_program: Interface<'info, TokenInterface>,
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

#[derive(Accounts)]
#[instruction(company_name:String)]
pub struct ClaimTokens<'info> {
    #[account(mut)]
    pub beneficiary: Signer<'info>,
    #[account(
        mut,
        has_one = beneficiary,
        has_one = vesting_account,
         seeds = [b"employee_vesting", beneficiary.key().as_ref(),vesting_account.key().as_ref()],
         bump =employee_account.bump
    )]
    pub employee_account: Account<'info, EmployeeAccount>,

    #[account(
        mut,
        has_one = treasury_account,
        has_one = mint,
        seeds = [b"vesting_treasury", company_name.as_bytes()],
        bump = vesting_account.bump,
        )]
    pub vesting_account: Account<'info, VestingAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub treasury_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = beneficiary,
        associated_token::mint = mint,
        associated_token::authority = beneficiary,
        associated_token::token_program = token_program,
    )]
    pub employee_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,

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
    pub beneficiary: Pubkey,
    pub start_time: i64,
    pub end_time: i64,
    pub cliff_time: i64, //time how much an employee has to wait before claming
    pub vesting_account: Pubkey,
    pub total_amount: u64,
    pub total_claimed: u64,
    pub bump: u8,
}


#[error_code]
pub enum ErrorCode {
    #[msg("Claim not available")]
    ClaimNotAvailable,
    #[msg("Vesting period ended")]
    VestingPeriodEnded,
    #[msg("Invalid vesting period")]
    InvalidVestingPeriod,
    #[msg("Overflow error")]
    OverflowError,
    #[msg("No tokens to claim")]
    NoTokensToClaim,
}