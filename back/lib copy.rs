use anchor_lang::prelude::*;
use anchor_spl::associated_token::{ self, Create, AssociatedToken };
use anchor_spl::token::{ self, Transfer, Mint, Token, TokenAccount };

declare_id!("3NGKZVHz9XEScGb48kC64KVuAJAq2cedSuVx6WhsgQV8");

#[program]
pub mod lottery {
    use super::*;

    pub fn create_app_stats(ctx: Context<CreateAppStats>, fee_percent: u8, bump: u8) -> Result<()> {
        let app_stats = &mut ctx.accounts.app_stats;
        app_stats.owner = ctx.accounts.signer.key();
        app_stats.fee_account = ctx.accounts.fee_account.key();
        app_stats.fee_percent = fee_percent;
        app_stats.bump = bump;

        Ok(())
    }

    pub fn update_app_stats(ctx: Context<UpdateAppStats>, fee_percent: u8) -> Result<()> {
        let app_stats = &mut ctx.accounts.app_stats;
        app_stats.fee_account = ctx.accounts.fee_account.key();
        app_stats.fee_percent = fee_percent;

        Ok(())
    }

    pub fn create_lottery(
        ctx: Context<CreateLottery>,
        price: u64,
        ticket_amount: u8,
        end: i64,
        prize_bump: u8,
        proceeds_bump: u8
    ) -> Result<()> {
        if price == 0 || ticket_amount <= 0{
            return err!(ErrCode::InvalidArgus);
        }
        let now = ctx.accounts.clock.unix_timestamp as i64;
        if end <= now {
            return err!(ErrCode::InvalidArgus);
        }

        let lottery = &mut ctx.accounts.lottery;
        lottery.ticket_price = price;
        lottery.ticket_amount = ticket_amount;
        lottery.left_tickets = (1..=lottery.ticket_amount).collect();
        lottery.start = now;
        lottery.end = end;
        lottery.creator = ctx.accounts.signer.key();
        lottery.prize_token = ctx.accounts.mint.key();
        lottery.prize_bump = prize_bump;
        lottery.proceeds_bump = proceeds_bump;
        Ok(())
    }

    pub fn buy_tickets(ctx: Context<BuyTickets>, amount: u64) -> Result<()> {
        let lottery: &mut Account<'_, Lottery> = &mut ctx.accounts.lottery;
        let ticket_price:u64 = lottery.ticket_price;
        //let fee_amount: u64 = (amount as u64 * price * (ctx.accounts.app_stats.fee_percent as u64)) / 100;
        //let real_amount:u64 = (amount as u64) * price - fee_amount;

        let now = ctx.accounts.clock.unix_timestamp as i64;
        let end_ts = lottery.end;

        // check duration sanity
        if now > end_ts {
            return err!(ErrCode::InvalidSchedule);
        }

        // check available tickets
        if amount == 0 || amount > (lottery.left_tickets.len() as u64) {
            return err!(ErrCode::InvalidArgus);
        }

        // if ctx.accounts.signer.to_account_info().lamports() < real_amount {
        //     return err!(ErrCode::InvalidFund);
        // }

        // send token
        let cpi_accounts = Transfer {
            from: ctx.accounts.creator_token.to_account_info(),
            to: ctx.accounts.prize.to_account_info(),
            authority: ctx.accounts.signer.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, ticket_price)?;

        // get random number
        for _ in 0..amount {
            let slot = ctx.accounts.clock.unix_timestamp as i64;
            let random_number = (slot as usize) % lottery.left_tickets.len();

            // assign ticket for signer
            let buyer = Buyer {
                participant: ctx.accounts.signer.key(),
                ticket_no: lottery.left_tickets[random_number],
            };
            lottery.buyers.push(buyer);
            lottery.left_tickets.remove(random_number);
        }
        // lottery.collected += real_amount;
        lottery.collected += amount;
        Ok(())
    }

    pub fn reveal_winner(ctx: Context<RevealWinner>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let now = ctx.accounts.clock.unix_timestamp as i64;
        let end_ts = lottery.end;

        // check duration sanity
        // if (now < end_ts) && (lottery.buyers.len() < (lottery.ticket_amount as usize)) {
        //     return err!(ErrCode::InvalidSchedule);
        // }

        if lottery.buyers.len() == 0 {
            return err!(ErrCode::NoBuyer);
        }

        // get random number
        let slot = ctx.accounts.clock.unix_timestamp as i64;
        let random_number = slot % (lottery.buyers.len() as i64);
        lottery.winner = lottery.buyers[random_number as usize].participant;
        Ok(())
    }

    pub fn claim_prize(ctx: Context<ClaimPrize>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        if lottery.winner != ctx.accounts.user.key() {
            return err!(ErrCode::InvalidWinner);
        }

        if lottery.claimed_amount > 0 {
            return err!(ErrCode::AlreadyClaimd);
        }

        // calculate amount
        let sold_tickets = lottery.ticket_amount - (lottery.left_tickets.len() as u8);
        let amount = lottery.ticket_price * sold_tickets as u64;
        //let amount = (lottery.price * (sold_tickets as u64)) / (lottery.ticket_amount as u64);
        //let amount = (lottery.prize_amount * (sold_tickets as u64)) / (lottery.ticket_amount as u64);
        //lottery.claimed_amount = amount;
        lottery.claimed_amount = amount;

        // format recipient token account if empty
        if ctx.accounts.user_token.data_is_empty() {
            let cpi_accounts = Create {
                payer: ctx.accounts.user.to_account_info(),
                associated_token: ctx.accounts.user_token.clone(),
                authority: ctx.accounts.user.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };
            let cpi_program = ctx.accounts.associated_token_program.to_account_info();
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
            associated_token::create(cpi_ctx)?;
        }

        // send token
        let nonce: u8 = lottery.prize_bump;
        let lottery = &mut ctx.accounts.lottery;
        let binding: Pubkey = lottery.key();
        let seeds: &[&[u8]; 3] = &[b"prize".as_ref(), binding.as_ref(), &[nonce]];
        let signer: &[&[&[u8]]; 1] = &[&seeds[..]];

        let cpi_accounts: Transfer<'_> = Transfer {
            from: ctx.accounts.prize.to_account_info(),
            to: ctx.accounts.user_token.to_account_info(),
            authority: ctx.accounts.prize.to_account_info(),
        };
        let cpi_program: AccountInfo<'_> = ctx.accounts.token_program.to_account_info();
        let cpi_ctx: CpiContext<'_, '_, '_, '_, Transfer<'_>> = CpiContext::new(cpi_program, cpi_accounts).with_signer(signer);
        token::transfer(cpi_ctx, lottery.claimed_amount)?;
        Ok(())
    }

    // pub fn collect_proceed(ctx: Context<CollectProceed>) -> Result<()> {
    //     let lottery = &mut ctx.accounts.lottery;

    //     if lottery.winner != ctx.accounts.system_program.key() && lottery.claimed_amount == 0 {
    //         return err!(ErrCode::InvalidSchedule);
    //     }

    //     if lottery.winner != ctx.accounts.system_program.key() {
    //         let amount: u64 = lottery.collected;
    //         let nonce: u8 = lottery.proceeds_bump;
    //         let binding: Pubkey = lottery.key();
    //         let seeds: &[&[u8]; 3] = &[b"proceeds".as_ref(), binding.as_ref(), &[nonce]];
    //         let signer: &[&[&[u8]]; 1] = &[&seeds[..]];
    //         let fee_amount: u64 = amount * (ctx.accounts.app_stats.fee_percent as u64) / 100;
    //         // send fee
    //         let fee_ix = anchor_lang::solana_program::system_instruction::transfer(
    //             &ctx.accounts.proceeds.key(),
    //             &ctx.accounts.fee_account.key(),
    //             fee_amount
    //         );

    //         anchor_lang::solana_program::program::invoke_signed(
    //             &fee_ix,
    //             &[
    //                 ctx.accounts.proceeds.to_account_info(),
    //                 ctx.accounts.fee_account.to_account_info(),
    //             ],
    //             signer
    //         )?;
    //         // collect sol
    //         let ix = anchor_lang::solana_program::system_instruction::transfer(
    //             &ctx.accounts.proceeds.key(),
    //             &ctx.accounts.creator.key(),
    //             amount - fee_amount
    //         );

    //         anchor_lang::solana_program::program::invoke_signed(
    //             &ix,
    //             &[ctx.accounts.proceeds.to_account_info(), ctx.accounts.creator.to_account_info()],
    //             signer
    //         )?;
    //     }

    //     // collect token
    //     let refundable = lottery.prize_amount - lottery.claimed_amount;
    //     if refundable > 0 {
    //         let nonce: u8 = lottery.prize_bump;
    //         let binding: Pubkey = lottery.key();
    //         let seeds: &[&[u8]; 3] = &[b"prize".as_ref(), binding.as_ref(), &[nonce]];
    //         let signer: &[&[&[u8]]; 1] = &[&seeds[..]];

    //         let cpi_accounts: Transfer<'_> = Transfer {
    //             from: ctx.accounts.prize.to_account_info(),
    //             to: ctx.accounts.user_token.to_account_info(),
    //             authority: ctx.accounts.prize.to_account_info(),
    //         };
    //         let cpi_program: AccountInfo<'_> = ctx.accounts.token_program.to_account_info();
    //         let cpi_ctx: CpiContext<'_, '_, '_, '_, Transfer<'_>> = CpiContext::new(cpi_program, cpi_accounts).with_signer(signer);
    //         token::transfer(cpi_ctx, lottery.prize_amount - lottery.claimed_amount)?;
    //     }
    //     // close account
    //     Ok(())
    // }
}

#[account]
pub struct AppStats {
    pub fee_account: Pubkey,
    pub fee_percent: u8,
    pub owner: Pubkey,
    bump: u8,
}

#[derive(Accounts)]
pub struct CreateAppStats<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// CHECK:don't read and write this account
    pub fee_account: AccountInfo<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + 32 * 2 + 2,
        seeds = [b"app-stats", signer.key().as_ref()],
        bump
    )]
    pub app_stats: Account<'info, AppStats>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateAppStats<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// CHECK:don't read and write this account
    pub fee_account: AccountInfo<'info>,

    #[account(
        mut, 
        constraint=app_stats.owner == signer.key(),
        seeds = [b"app-stats", signer.key().as_ref()],
        bump = app_stats.bump
    )]
    pub app_stats: Account<'info, AppStats>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateLottery<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(init, payer = signer, space = 8 + 3000)]
    pub lottery: Box<Account<'info, Lottery>>,

    #[account(
        init,
        seeds = [b"prize", lottery.key().as_ref()],
        bump,
        payer = signer,
        owner = token_program.key(),
        rent_exempt = enforce,
        token::mint = mint,
        token::authority = prize
    )]
    pub prize: Account<'info, TokenAccount>,

    #[account(seeds = [b"proceeds", lottery.key().as_ref()], bump)]
    pub proceeds: SystemAccount<'info>,
    pub clock: Sysvar<'info, Clock>,
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Lottery {
    pub creator: Pubkey,
    pub start: i64,
    pub end: i64,
    pub winner: Pubkey,
    pub ticket_price: u64,
    pub ticket_amount: u8,
    pub prize_token: Pubkey,
    pub claimed_amount: u64,
    pub buyers: Vec<Buyer>,
    pub left_tickets: Vec<u8>,
    pub prize_bump: u8,
    pub proceeds_bump: u8,
    pub collected: u64,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct Buyer {
    pub participant: Pubkey,
    pub ticket_no: u8,
}

#[derive(Accounts)]
pub struct BuyTickets<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,

    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut, seeds = [b"proceeds", lottery.key().as_ref()], bump = lottery.proceeds_bump)]
    pub proceeds: SystemAccount<'info>,

    #[account(
        mut, 
        seeds = [b"app-stats", owner.key().as_ref()], 
        bump = app_stats.bump, 
        constraint = fee_account.key() == app_stats.fee_account
    )]
    pub app_stats: Account<'info, AppStats>,

    #[account(
        mut,
        constraint = creator_token.mint == lottery.prize_token
    )]
    pub creator_token: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"prize", lottery.key().as_ref()],
        bump = lottery.prize_bump
    )]
    pub prize: Account<'info, TokenAccount>,
    // #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// CHECK: don't read and write this account
    #[account(mut)]
    pub fee_account: AccountInfo<'info>,

    /// CHECK: don't read and write this account
    pub owner: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RevealWinner<'info> {
    #[account(mut)]
    pub lottery: Account<'info, Lottery>,
    pub clock: Sysvar<'info, Clock>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimPrize<'info> {
    #[account(mut, constraint = lottery.prize_token == mint.key())]
    pub lottery: Account<'info, Lottery>,

    #[account(mut)]
    pub user: Signer<'info>,

    ///CHECK: don't read write this contract
    #[account(mut)]
    pub user_token: AccountInfo<'info>,

    #[account(mut)]
    pub prize: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CollectProceed<'info> {
    #[account(mut, constraint = lottery.creator == creator.key(), close=creator)]
    pub lottery: Account<'info, Lottery>,

    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(mut)]
    pub user_token: Account<'info, TokenAccount>,

    #[account(mut)]
    pub prize: Account<'info, TokenAccount>,

    #[account(mut)]
    pub proceeds: SystemAccount<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut, 
        seeds = [b"app-stats", owner.key().as_ref()], 
        bump = app_stats.bump, 
        constraint = fee_account.key() == app_stats.fee_account
    )]
    pub app_stats: Account<'info, AppStats>,
    
    /// CHECK: don't read and write this account
    #[account(mut)]
    pub fee_account: AccountInfo<'info>,

    /// CHECK: don't read and write this account
    pub owner: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum ErrCode {
    #[msg("Invalid vesting schedule given.")]
    InvalidSchedule,
    #[msg("Invalid associated token address. Did you provide the correct address?")]
    InvalidAssociatedTokenAddress,
    #[msg("Insufficient fund")]
    InvalidFund,
    #[msg("Invalid unlock time")]
    InvalidUnlockTime,
    #[msg("Invalid unlock amount")]
    InvalidUnlockAmount,
    #[msg("You are not winner!")]
    InvalidWinner,
    #[msg("Already claimed!")]
    AlreadyClaimd,
    #[msg("Invalid arguments")]
    InvalidArgus,
    #[msg("No buyer")]
    NoBuyer,
}


// #[program]
// pub mod lottery {
//     use super::*;

//     pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
//         Ok(())
//     }
// }



// #[derive(Accounts)]
// pub struct Initialize {}
