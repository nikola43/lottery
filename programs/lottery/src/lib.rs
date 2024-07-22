use anchor_lang::prelude::*;
//use anchor_spl::associated_token::{ self, Create, AssociatedToken };
// use anchor_spl::token::{ self, Transfer, Mint, Token, TokenAccount };

use anchor_spl::{
    token_interface::{TokenAccount, Mint, MintTo, mint_to,
    CloseAccount, close_account,   
    TokenInterface, TransferChecked, transfer_checked}, 
    associated_token::AssociatedToken};


//use anchor_spl::token_2022::{ Transfer};

mod randomness_tools;
use randomness_tools::get_sha256_hashed_random;
declare_id!("E5Tmweyj2XLDn1L746PPdt7dAbG397qvTj8wYBqEaBSX");

#[program]
pub mod lottery {
    use super::*;

    pub fn create_app_stats(ctx: Context<CreateAppStats>, fee_percent: u8, bump: u8) -> Result<()> {
        let app_stats = &mut ctx.accounts.app_stats;
        app_stats.owner = ctx.accounts.signer.key();
        app_stats.admin = ctx.accounts.admin_account.key();
        app_stats.fee_account = ctx.accounts.fee_account.key();
        app_stats.fee_percent = fee_percent;
        app_stats.current_round = 0;
        app_stats.current_round_key = Pubkey::default();
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
        ticket_price: u64,
        ticket_amount: u8,
        prize_bump: u8,
        proceeds_bump: u8
    ) -> Result<()> {
        if ticket_price == 0 || ticket_amount <= 0{
            return err!(ErrCode::InvalidArgus);
        }
        let now = ctx.accounts.clock.unix_timestamp as i64;
        let end = now + 60 * 60 * 24; // 24 hours        

        let lottery = &mut ctx.accounts.lottery;
        lottery.ticket_price = ticket_price;
        lottery.ticket_amount = ticket_amount;
        lottery.left_tickets = (1..=lottery.ticket_amount).collect();
        lottery.start = now;
        lottery.end = end;
        lottery.creator = ctx.accounts.signer.key();
        lottery.prize_token = ctx.accounts.mint.key();
        lottery.prize_bump = prize_bump;
        lottery.proceeds_bump = proceeds_bump;
        lottery.max_tickets_per_buyer = 5;

        let app_stats = &mut ctx.accounts.app_stats;
        app_stats.current_round += 1;
        app_stats.current_round_key = lottery.key();
        
        Ok(())
    }

    pub fn buy_tickets(ctx: Context<BuyTickets>, ticket_amount: u64) -> Result<()> {
        let lottery: &mut Account<'_, Lottery> = &mut ctx.accounts.lottery;
        let ticket_price:u64 = lottery.ticket_price;
        
        let now = ctx.accounts.clock.unix_timestamp as i64;
        let end_ts = lottery.end;

        // check duration sanity
        if now > end_ts {
            return err!(ErrCode::RoundEnded);
        }

        // check available tickets
        if ticket_amount == 0 || ticket_amount > (lottery.left_tickets.len() as u64) {
            return err!(ErrCode::InvalidArgus);
        }

        //let total_amount = amount * ticket_price;
        let fee_amount: u64 = (ticket_amount as u64 * ticket_price * (ctx.accounts.app_stats.fee_percent as u64)) / 100;
        let real_amount:u64 = (ticket_amount as u64) * ticket_price - fee_amount;

        // transfer fee to fee account
        // let cpi_accounts = Transfer {
        //     from: ctx.accounts.creator_token.to_account_info(),
        //     to: ctx.accounts.fee_account.to_account_info(),
        //     authority: ctx.accounts.signer.to_account_info(),
        // };
        // let cpi_program = ctx.accounts.token_program.to_account_info();
        // let cpi_ctx: CpiContext<Transfer> = CpiContext::new(cpi_program, cpi_accounts);
        // token::transfer(cpi_ctx, fee_amount)?;


        // transfer tokens from buyer to prize account
        // let cpi_accounts = Transfer {
        //     from: ctx.accounts.creator_token.to_account_info(),
        //     to: ctx.accounts.prize.to_account_info(),
        //     authority: ctx.accounts.signer.to_account_info(),
        // };
        // let cpi_program = ctx.accounts.token_program.to_account_info();
        // let cpi_ctx: CpiContext<Transfer> = CpiContext::new(cpi_program, cpi_accounts);
        //token::transfer(cpi_ctx, real_amount)?;

        for _ in 0..ticket_amount {
            let slot = ctx.accounts.clock.unix_timestamp as u64;
            let n = get_sha256_hashed_random(slot,lottery.left_tickets.len() as u64);
            let random_number = (n as usize) % lottery.left_tickets.len();

            // Determine the index of the existing buyer, if any
            let buyer_index = lottery.buyers.iter().position(|buyer| buyer.participant == ctx.accounts.signer.key());

            // Clone the ticket number to be added since it will be used in both match arms
            let ticket_to_add = lottery.left_tickets[random_number].clone();

            match buyer_index {
                Some(index) => {
                    // Temporarily remove the buyer to bypass the borrow checker
                    let mut buyer = lottery.buyers.remove(index);

                    // check if the buyer has reached the maximum number of tickets
                    if buyer.tickets.len() + ticket_amount as usize >= lottery.max_tickets_per_buyer as usize {
                        return err!(ErrCode::MaxTicketsPerBuyer);
                    }

                    buyer.tickets.push(ticket_to_add);
                    // Re-insert the buyer back at the same index
                    lottery.buyers.insert(index, buyer);
                },
                None => {
                    // check if the buyer has reached the maximum number of tickets
                    if ticket_amount as usize >= lottery.max_tickets_per_buyer as usize {
                        return err!(ErrCode::MaxTicketsPerBuyer);
                    }
                    // If the buyer does not exist, create a new buyer and add them to the list
                    let buyer = Buyer {
                        participant: ctx.accounts.signer.key(),
                        tickets: vec![ticket_to_add],
                    };
                    lottery.buyers.push(buyer);
                },
            }
            
            lottery.left_tickets.remove(random_number);
        }
        // lottery.collected += real_amount;
        lottery.collected += ticket_price * ticket_amount;
        Ok(())
    }

    pub fn reveal_winners(ctx: Context<RevealWinner>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;
        let now = ctx.accounts.clock.unix_timestamp as i64;
        let end_ts = lottery.end;

        // check duration sanity
        // if (now < end_ts) && (lottery.buyers.len() < (lottery.ticket_amount as usize)) {
        //     return err!(ErrCode::InvalidSchedule);
        // }

        let buyers_len = lottery.buyers.len(); 

        // Check if there are no buyers
        if buyers_len == 0 {
            return err!(ErrCode::BuyerListEmpty);
        }

        let slot = ctx.accounts.clock.unix_timestamp as u64;
        let mut temp_winners: Vec<Winner> = Vec::new(); // Temporary vector to hold winners

        for _ in 0..(buyers_len / 2) {
            // let random_number = slot % (buyers_len as i64);
            let n = get_sha256_hashed_random(slot,lottery.left_tickets.len() as u64);
            let random_number = (n as usize) % lottery.left_tickets.len();
            let winner = Winner {
                participant: lottery.buyers[random_number as usize].participant,
                claimed: false,
                claimed_amount: 0,
            };
            temp_winners.push(winner);
        }

        lottery.winners.extend(temp_winners);

        Ok(())        
    }

    pub fn claim_prize(ctx: Context<ClaimPrize>) -> Result<()> {
        let lottery = &mut ctx.accounts.lottery;

        // check if user is on the winner list
        let mut is_winner = false;
        let mut winner_index = 0;
        for (index, winner) in lottery.winners.iter().enumerate() {
            if winner.participant == ctx.accounts.user.key() {
                is_winner = true;
                winner_index = index;
                break;
            }
        }

        if !is_winner {
            return err!(ErrCode::InvalidWinner);
        }

        // check if prize is already claimed
        if lottery.winners[winner_index].claimed {
            return err!(ErrCode::AlreadyClaimd);
        }
        lottery.winners[winner_index].claimed = true;
        

        // if lottery.claimed_amount > 0 {
        //     return err!(ErrCode::AlreadyClaimd);
        // }

        // calculate amount
        //let sold_tickets = lottery.ticket_amount - (lottery.left_tickets.len() as u8);
        //let amount = lottery.ticket_price * sold_tickets as u64;
        //let amount = (lottery.price * (sold_tickets as u64)) / (lottery.ticket_amount as u64);
        //let amount = (lottery.prize_amount * (sold_tickets as u64)) / (lottery.ticket_amount as u64);
        //lottery.claimed_amount = amount;

        // calculate amount, for now we will use the collected amount divided number of winners
        let amount = lottery.collected / (lottery.winners.len() as u64);
        lottery.claimed_amount += amount;

        // format recipient token account if empty
        // if ctx.accounts.user_token.data_is_empty() {
        //     let cpi_accounts = Create {
        //         payer: ctx.accounts.user.to_account_info(),
        //         associated_token: ctx.accounts.user_token.clone(),
        //         authority: ctx.accounts.user.to_account_info(),
        //         mint: ctx.accounts.mint.to_account_info(),
        //         system_program: ctx.accounts.system_program.to_account_info(),
        //         token_program: ctx.accounts.token_program.to_account_info(),
        //     };
        //     let cpi_program = ctx.accounts.associated_token_program.to_account_info();
        //     let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        //     associated_token::create(cpi_ctx)?;
        // }

        // // send token
        // let nonce: u8 = lottery.prize_bump;
        // let lottery = &mut ctx.accounts.lottery;
        // let binding: Pubkey = lottery.key();
        // let seeds: &[&[u8]; 3] = &[b"prize".as_ref(), binding.as_ref(), &[nonce]];
        // let signer: &[&[&[u8]]; 1] = &[&seeds[..]];

        // let cpi_accounts: Transfer<'_> = Transfer {
        //     from: ctx.accounts.prize.to_account_info(),
        //     to: ctx.accounts.user_token.to_account_info(),
        //     authority: ctx.accounts.prize.to_account_info(),
        // };
        // let cpi_program: AccountInfo<'_> = ctx.accounts.token_program.to_account_info();
        // let cpi_ctx: CpiContext<'_, '_, '_, '_, Transfer<'_>> = CpiContext::new(cpi_program, cpi_accounts).with_signer(signer);
        //token::transfer(cpi_ctx, lottery.claimed_amount)?;
        Ok(())
    }

    
}

#[account]
pub struct AppStats {
    pub fee_account: Pubkey,
    pub fee_percent: u8,
    pub owner: Pubkey,
    pub admin: Pubkey,
    pub current_round: u64,
    pub current_round_key: Pubkey,
    bump: u8,
}

#[derive(Accounts)]
pub struct CreateAppStats<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
   
    /// CHECK:don't read and write this account
    pub admin_account:  AccountInfo<'info>,

    /// CHECK:don't read and write this account
    pub fee_account: AccountInfo<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + 32 * 2 + 2 + 32 + 8 + 32,
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
    pub prize:  InterfaceAccount<'info, TokenAccount>,

    #[account(seeds = [b"proceeds", lottery.key().as_ref()], bump)]
    pub proceeds: SystemAccount<'info>,
    pub clock: Sysvar<'info, Clock>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub app_stats: Account<'info, AppStats>,
}

#[account]
pub struct Lottery {
    pub creator: Pubkey,
    pub start: i64,
    pub end: i64,
    pub winners: Vec<Winner>,
    pub ticket_price: u64,
    pub ticket_amount: u8,
    pub prize_token: Pubkey,
    pub claimed_amount: u64,
    pub buyers: Vec<Buyer>,
    pub left_tickets: Vec<u8>,
    pub prize_bump: u8,
    pub proceeds_bump: u8,
    pub collected: u64,
    pub max_tickets_per_buyer: u8,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct Buyer {
    pub participant: Pubkey,
    pub tickets: Vec<u8>,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct Winner {
    pub participant: Pubkey,
    pub claimed: bool,
    pub claimed_amount: u64,
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
    pub creator_token:  InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"prize", lottery.key().as_ref()],
        bump = lottery.prize_bump
    )]
    pub prize:  InterfaceAccount<'info, TokenAccount>,
    // #[account(address = token::ID)]
    pub token_program: Interface<'info, TokenInterface>,


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
    pub prize: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
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
    pub user_token:  InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub prize:  InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub proceeds: SystemAccount<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

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

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum ErrCode {
    #[msg("Lottery round ended")]
    RoundEnded,
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
    #[msg("Buyers list is empty")]
    BuyerListEmpty,
    #[msg("Maximum tickets per buyer reached")]
    MaxTicketsPerBuyer,
}