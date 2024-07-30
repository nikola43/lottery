use anchor_lang::prelude::*;
use anchor_spl::associated_token::{ self, Create, AssociatedToken };
use anchor_spl::token::{ self, Transfer, Mint, Token, TokenAccount };
use std::collections::HashMap;
mod randomness_tools;
use randomness_tools::get_sha256_hashed_random;
declare_id!("B1xS2zq7MFpNuQvzm7VYjr54JUuaMrxCoVjRtw2oJvMr");

/*
collect fees when user buys ticket
unresolved case
check before start lottery
use oracle
reduce fees from collected amount

reveal winners fxn => 
if 25 users, send the one ticket fee to owner or fee account
if tickets bought<10, they can withdraw amount-fees
update buyer_length to total tickets

proper cases
A. if 10 people buy 5 tickets = 50 tickets in total
winning tickets chosen will be 25

B. if each ticket is 1$, and 100 tickets are sold, than there is total 100$-fees in the lottery
winners will be 50% of tickets sold is 50 winners
and that (100$-fees) should be distributed to 50 people


in claim_prize =>
if lottery did not run, in case the tickets sold <10
we allow the user to claim the amount

it should consider multiple lottery rounds, say

lottery 1 -> win 10 tickets
lottery 2 -> did not go well, withdraw just ticket amount- fees
lottery 3 -> win 20 tickets
later if user call claim_prize {he should withdraw amount against all these above tickets at once}

*/


#[program]
pub mod lottery {
    use super::*;

    /**
     * Initialize the app stats account
     * @param ctx is the context of the program
     * @param fee_percent is the fee percentage to be charged
     * @param bump is the bump seed for the account
     * @return the result of the operation
     */
    pub fn create_app_stats(ctx: Context<CreateAppStats>, fee_percent: u8, bump: u8) -> Result<()> {
        let app_stats = &mut ctx.accounts.app_stats;
        app_stats.owner = ctx.accounts.signer.key();
        //app_stats.admin = ctx.accounts.admin_account.key();
        app_stats.fee_account = ctx.accounts.fee_account.key();
        app_stats.fee_percent = fee_percent;
        app_stats.rounds = Vec::new();
        //app_stats.mint = ctx.accounts.mint.key();
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
        // Validations, check if the caller is admin, if ticket price is not zero, if ticket amount is not zero
        if ctx.accounts.app_stats.owner != ctx.accounts.signer.key() {
            return err!(ErrCode::CallerIsNotAdmin);
        }
        if ticket_price == 0{
            return err!(ErrCode::InvalidTicketPrice);
        }
        if  ticket_amount <= 0 {
            return err!(ErrCode::InvalidTicketAmount);
        }      

        let lottery = &mut ctx.accounts.lottery;
        lottery.ticket_price = ticket_price;
        lottery.ticket_amount = ticket_amount;
        lottery.left_tickets = (1..=lottery.ticket_amount).collect();
        lottery.start = ctx.accounts.clock.unix_timestamp as i64;
        lottery.end = ctx.accounts.clock.unix_timestamp as i64 + 60 * 60 * 24; // 24 hours
        lottery.creator = ctx.accounts.signer.key();
        lottery.prize_token = ctx.accounts.mint.key();
        lottery.prize_bump = prize_bump;
        lottery.proceeds_bump = proceeds_bump;
        lottery.max_tickets_per_buyer = 5;
        lottery.status = LotteryStatus::Running;

        let app_stats = &mut ctx.accounts.app_stats;
        let mut current_round = app_stats.rounds.len();
        msg!("Current round: {}", current_round);
        app_stats.rounds.push(lottery.key());
        current_round = app_stats.rounds.len();
        msg!("Current round: {}", current_round);
        msg!("Current round key: {}", app_stats.rounds[current_round - 1]);
        
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

        ///transfer fee to fee account
        // let cpi_accounts = Transfer {
        //     from: ctx.accounts.creator_token.to_account_info(),
        //     to: ctx.accounts.fee_account.to_account_info(),
        //     authority: ctx.accounts.signer.to_account_info(),
        // };
        // let cpi_program = ctx.accounts.token_program.to_account_info();
        // let cpi_ctx: CpiContext<Transfer> = CpiContext::new(cpi_program, cpi_accounts);
        // token::transfer(cpi_ctx, fee_amount)?;


        // transfer tokens from buyer to prize account
        let cpi_accounts = Transfer {
            from: ctx.accounts.creator_token.to_account_info(),
            to: ctx.accounts.prize.to_account_info(),
            authority: ctx.accounts.signer.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx: CpiContext<Transfer> = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, real_amount)?;

        for _ in 0..ticket_amount {
            // todo update this with RGN 
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
        lottery.collected += ticket_price * ticket_amount; // todo: less fees
        Ok(())
    }

    pub fn reveal_winners(ctx: Context<RevealWinner>) -> Result<()> {
        // check lottery status
        let lottery = &mut ctx.accounts.lottery;
        let now = ctx.accounts.clock.unix_timestamp as i64;
        let end_ts = lottery.end;
        if now > end_ts {
            // TODO: if tickets sold are lower than 10, we set lottery as unresolved
            // and allow users withdraw his tickets 
            if lottery.ticket_amount as usize - lottery.left_tickets.len() < 10 {
                lottery.status = LotteryStatus::Unresolved;
            } else {
                lottery.status = LotteryStatus::Ended;
            }
        } 

        // check if buyers list is empty
        let buyers_len = lottery.buyers.len(); 
        if buyers_len == 0 {
            //return err!(ErrCode::BuyerListEmpty);
            lottery.status = LotteryStatus::Unresolved;
            return Ok(());
        }

        let slot = ctx.accounts.clock.unix_timestamp as u64;
        let mut temp_winners: Vec<Winner> = Vec::new(); // Temporary vector to hold winners

        // TODO: if buyers_len is odd, we will send one ticket to fee account
        for _ in 0..(buyers_len / 2) {
            let random_number = slot % (buyers_len as u64);
            // let n = get_sha256_hashed_random(slot,lottery.left_tickets.len() as u64);
            // let random_number = (n as usize) % lottery.left_tickets.len();
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
        let app_stats = &mut ctx.accounts.app_stats;
        let current_round = app_stats.rounds.len();
        //let current_round_key = app_stats.rounds[current_round - 1];
        msg!("Current round: {:?}", app_stats.rounds);
        //msg!("Current round key: {}", current_round_key);

        let fee_percent = app_stats.fee_percent;
        let mut claimable_amount = 10;

 


        for round in 0..current_round {
            let round_key: Pubkey = app_stats.rounds[round as usize];

            // Define the seeds used to derive the PDA
            let seeds = &[b"lottery", round_key.as_ref()];
            let (lottery_pda, _bump_seed) = Pubkey::find_program_address(seeds, ctx.program_id);

            // Find the lottery account info from the remaining accounts
            let lottery_account_info = ctx.remaining_accounts.iter().find(|account| account.key == &lottery_pda).ok_or(ProgramError::InvalidAccountData)?;

            // Fetch the lottery account data
            let lottery_data = &lottery_account_info.try_borrow_data()?;
            let mut lottery: Lottery = Lottery::try_from_slice(&lottery_data)?;

            //claimable_amount += lottery.collected as i32;
            //lottery.collected = 0;

            msg!("Number of buyers: {}", lottery.buyers.len());
            msg!("Number of winner: {}", lottery.winners.len());
        }


    
        // loop through lottery rounds
        // for round in 0..app_stats.current_round {
        //     let round_key = app_stats.current_round_list[round as usize];
        //     // Get a mutable reference to the lottery
        //     //let lottery = app_stats.lotteries.get_mut(0).unwrap();
        //     //let lottery = app_stats.lotteries.get_mut(&round_key).unwrap();

        //     // check if lottery is running, if yes, skip
        //     if lottery.status == LotteryStatus::Running {
        //         continue;
        //     }

        //     // check if lottery is unresolved
        //     if lottery.status == LotteryStatus::Unresolved {
        //         // check if user is on the buyer list
        //         let mut is_buyer = false;
        //         let mut buyer_index = 0;
        //         for (index, buyer) in lottery.buyers.iter().enumerate() {
        //             if buyer.participant == ctx.accounts.user.key() {
        //                 is_buyer = true;
        //                 buyer_index = index;
        //                 break;
        //             }
        //         }

        //         if !is_buyer {
        //             return err!(ErrCode::InvalidBuyer);
        //         }

        //         // calculate claimable amount, will be ticket price * number of tickets bought
        //         claimable_amount += lottery.ticket_price * (lottery.buyers[buyer_index].tickets.len() as u64);
        //         // substract fee from claimable amount
        //         claimable_amount -= (claimable_amount * fee_percent as u64) / 100;
        //         lottery.buyers.remove(buyer_index);
        //     }

        //     // check if lottery is ended
        //     if lottery.status == LotteryStatus::Ended {
        //         // check if user is on the winner list
        //         let mut is_winner = false;
        //         let mut winner_index = 0;
        //         for (index, winner) in lottery.winners.iter().enumerate() {
        //             if winner.participant == ctx.accounts.user.key() {
        //                 is_winner = true;
        //                 winner_index = index;
        //                 break;
        //             }
        //         }

        //         if !is_winner {
        //             return err!(ErrCode::InvalidWinner);
        //         }

        //         // check if prize is already claimed
        //         if lottery.winners[winner_index].claimed {
        //             continue;
        //         }

        //         // calculate claimable amount
        //         // calculate amount, for now we will use the collected amount divided number of winners
        //         // todo: we should calculate amount based on number of tickets sold, not number of winners
        //         let amount = lottery.collected / (lottery.winners.len() as u64);
        //         lottery.claimed_amount += amount;
        //         lottery.winners[winner_index].claimed = true;
        //         lottery.winners[winner_index].claimed_amount = amount;
        //     }
        // }

        // check if claimable amount is zero
        if claimable_amount == 0 {
            return err!(ErrCode::ClaimableAmountIsZero);
        }

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
            // //let lottery = &mut ctx.accounts.lottery;
            // //let binding: Pubkey = lottery.key();
            // let binding: Pubkey = round_key;
            // let seeds: &[&[u8]; 3] = &[b"prize".as_ref(), binding.as_ref(), &[nonce]];
            // let signer: &[&[&[u8]]; 1] = &[&seeds[..]];

            // let cpi_accounts: Transfer<'_> = Transfer {
            //     from: ctx.accounts.prize.to_account_info(),
            //     to: ctx.accounts.user_token.to_account_info(),
            //     authority: ctx.accounts.prize.to_account_info(),
            // };
            // let cpi_program: AccountInfo<'_> = ctx.accounts.token_program.to_account_info();
            // let cpi_ctx: CpiContext<'_, '_, '_, '_, Transfer<'_>> = CpiContext::new(cpi_program, cpi_accounts).with_signer(signer);
            // token::transfer(cpi_ctx, lottery.claimed_amount)?;
        
            Ok(())
}  
}

#[account]
pub struct AppStats {
    pub fee_account: Pubkey,
    pub fee_percent: u8,
    pub owner: Pubkey,
    pub admin: Pubkey,
    pub rounds: Vec<Pubkey>,
    //pub lotteries: Vec<Lottery>,
    //pub lotteries: HashMap<Pubkey, Lottery>,
    //pub mint: Pubkey,
    bump: u8,
}

#[derive(Accounts)]
pub struct CreateAppStats<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
   
    // /// CHECK:don't read and write this account
    // pub admin_account:  AccountInfo<'info>,

    /// CHECK:don't read and write this account
    pub fee_account: AccountInfo<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = signer,
        space = 8 + 32 * 2 + 2 + 32 + 8 + 32 + 88,
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
    pub app_stats: Account<'info, AppStats>,
    //pub admin_account: AccountInfo<'info>,
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
    pub status: LotteryStatus,
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
pub struct UpdateLotteryStatus<'info> {
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
    pub app_stats: Account<'info, AppStats>,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize,PartialEq)]
pub enum LotteryStatus {
    Unresolved,
    Running,
    Ended,
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
    #[msg("You are not buyer!")]
    InvalidBuyer,
    #[msg("Already claimed!")]
    AlreadyClaimd,
    #[msg("Invalid arguments")]
    InvalidArgus,
    #[msg("Caller is not admin")]
    CallerIsNotAdmin,
    #[msg("Invalid ticket price")]
    InvalidTicketPrice,
    #[msg("Invalid ticket amount")]
    InvalidTicketAmount,
    #[msg("Claimable amount is zero")]
    ClaimableAmountIsZero,
    #[msg("Buyers list is empty")]
    BuyerListEmpty,
    #[msg("Maximum tickets per buyer reached")]
    MaxTicketsPerBuyer,
}