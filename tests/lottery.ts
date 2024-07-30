import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Lottery } from "../target/types/lottery";
import { PublicKey, LAMPORTS_PER_SOL, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { BN } from "bn.js";
import NodeWallet from "@coral-xyz/anchor/dist/cjs/nodewallet";
import { Account, ASSOCIATED_TOKEN_PROGRAM_ID, createAssociatedTokenAccountIdempotent, createAssociatedTokenAccountIdempotentInstruction, createMint, getOrCreateAssociatedTokenAccount, mintTo, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { expect } from "chai";

function sleep(ms: number) {
  return new Promise(resolve => setTimeout(resolve, ms))
}

describe("Lottery", () => {

  const provider = anchor.AnchorProvider.env();

  anchor.setProvider(provider);
  const connection = provider.connection;
  const program = anchor.workspace.Lottery as Program<Lottery>;

  const owner = provider.wallet as NodeWallet;
  const users = Array.from({ length: 10 }, () => anchor.web3.Keypair.generate());
  const usersAtas: Account[] = [];
  const feeAccount = anchor.web3.Keypair.generate()
  const adminAccount = anchor.web3.Keypair.generate()
  const lotteryAccount = anchor.web3.Keypair.generate();

  console.log({
    owner: owner.publicKey.toBase58(),
    feeAccount: feeAccount.publicKey.toBase58(),
    lotteryAccount: lotteryAccount.publicKey.toBase58()
  })

  const mintKeypairSC = anchor.web3.Keypair.generate();
  let mint = mintKeypairSC.publicKey;
  let ownerAta: Account
  let feeAccountAta: Account


  it("Should airdrop sol", async () => {
    await airdropSol(provider.connection, feeAccount.publicKey);
    await airdropSol(provider.connection, adminAccount.publicKey);

    const feeAccountSolBalance = await provider.connection.getBalance(feeAccount.publicKey);
    const adminAccountSolBalance = await provider.connection.getBalance(adminAccount.publicKey);

    expect(feeAccountSolBalance).to.be.equal(1 * LAMPORTS_PER_SOL);
    expect(adminAccountSolBalance).to.be.equal(1 * LAMPORTS_PER_SOL);

    for (let i = 0; i < users.length; i++) {
      await airdropSol(provider.connection, users[i].publicKey)
      const userSolBalance = await provider.connection.getBalance(users[i].publicKey);
      expect(userSolBalance).to.be.equal(1 * LAMPORTS_PER_SOL);
    }
  })

  it("Should create mint token", async () => {
    // Stablecoin mint
    await createMint(
      provider.connection,
      owner.payer,
      owner.publicKey,
      owner.publicKey,
      9,
      mintKeypairSC,
      undefined,
      TOKEN_PROGRAM_ID
    );

    // Initialise ATA
    ownerAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      users[0],
      mint,
      owner.publicKey
    );

    feeAccountAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      owner.payer,
      mint,
      feeAccount.publicKey
    );

    for (let i = 0; i < users.length; i++) {
      usersAtas[i] = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        users[i],
        mint,
        users[i].publicKey
      );

      await mintTo(
        provider.connection,
        owner.payer,
        mint,
        usersAtas[i].address,
        owner.payer,
        1000000000 * 10000, // 1000 tokens
        [],
        undefined,
        TOKEN_PROGRAM_ID
      );

      const userTokenBalance = await provider.connection.getTokenAccountBalance(usersAtas[i].address);
      //console.log("User token balance is ", userTokenBalance.value.uiAmount);
      expect(userTokenBalance.value.uiAmount).to.be.equal(10000);
    }
  })


  it("initalize", async () => {
    const fee_percent = 1;
    let [appStats, bump] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode('app-stats'),
        owner.publicKey.toBuffer(),
      ],
      program.programId
    );

    await program.methods.createAppStats(
      fee_percent,
      bump
    ).accounts({
      appStats,
      mint,
      feeAccount: feeAccount.publicKey,
      //adminAccount: adminAccount.publicKey
    }).rpc();

    // const appStatsData = await program.account.lottery.fetch(appStats);
    // console.log(appStatsData);
  });

  // it("update app stats", async () => {
  //   const fee_percent = 5;
  //   let [appStats, bump] = PublicKey.findProgramAddressSync(
  //     [
  //       anchor.utils.bytes.utf8.encode('app-stats'),
  //       owner.publicKey.toBuffer(),
  //     ],
  //     program.programId
  //   );

  //   await program.methods.updateAppStats(fee_percent).accounts({
  //     appStats,
  //     feeAccount: anchor.web3.Keypair.generate().publicKey
  //   }).rpc();

  //   const appStatsData = await program.account.lottery.fetch(appStats);
  //   console.log(appStatsData);
  // })



  it("Create competition", async () => {
    try {

      const ticketPrice = new BN(10 * Math.pow(10, 9)); // 10 tokens
      //console.log(ticketPrice.toString());
      //const ticketPrice = new BN(1);
      const ticketAmount = 100;
      //console.log(prizeAmount.toString());

      const [prize, prize_bump] = PublicKey.findProgramAddressSync(
        [anchor.utils.bytes.utf8.encode("prize"), lotteryAccount.publicKey.toBuffer()],
        program.programId
      );

      const [proceeds, proceeds_bump] = PublicKey.findProgramAddressSync(
        [anchor.utils.bytes.utf8.encode("proceeds"), lotteryAccount.publicKey.toBuffer()],
        program.programId
      );

      let [appStats, bump] = PublicKey.findProgramAddressSync(
        [
          anchor.utils.bytes.utf8.encode('app-stats'),
          owner.publicKey.toBuffer(),
        ],
        program.programId
      );

      const ixn1 = await program.methods.createLottery(
        ticketPrice,
        ticketAmount,
        prize_bump,
        proceeds_bump
      ).accounts({
        lottery: lotteryAccount.publicKey,
        mint,
        prize,
        proceeds,
        appStats
      }).signers([
        lotteryAccount,
        owner.payer
      ]).instruction();

      const { blockhash } = await connection.getLatestBlockhash();

      const message = new TransactionMessage({
        payerKey: owner.publicKey,
        recentBlockhash: blockhash,
        instructions: [ixn1]
      }).compileToV0Message();

      const transaction = new VersionedTransaction(message);

      transaction.sign([owner.payer, lotteryAccount]);

      await provider.connection.confirmTransaction(
        await provider.connection.sendRawTransaction(transaction.serialize())
      );

    } catch (error) {
      console.error(error);
      // if (error instanceof SendTransactionError) {
      //   const logs = error.getLogs(connection);
      //   console.log("Transaction Error Logs:", logs);
      // } else {
      //   console.error("An unexpected error occurred:", error);
      // }
    }
  });

  it("Should get lotteryInfo", async () => {
    const lotteryInfo = await program.account.lottery.fetch(lotteryAccount.publicKey);
    const ticketAmount = lotteryInfo.ticketAmount
    const leftTickets = lotteryInfo.leftTickets.length
    const ticketPrice = lotteryInfo.ticketPrice
    console.log({
      leftTickets,
      ticketAmount,
      ticketPrice: Number(ticketPrice.toString()) / Math.pow(10, 9)
    })
    //console.log(lotteryInfo);
  })
  it("Buy tickets", async () => {

    const [prize, prize_bump] = PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("prize"), lotteryAccount.publicKey.toBuffer()],
      program.programId
    );

    const [proceeds, proceeds_bump] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode('proceeds'),
        lotteryAccount.publicKey.toBuffer(),
      ],
      program.programId
    );

    const [appStats, bump] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode('app-stats'),
        owner.publicKey.toBuffer()
      ],
      program.programId
    );


    for (let i = 0; i < users.length; i++) {
      const tx1 = await program.methods.buyTickets(
        new BN(2)
      ).accounts({
        prize,
        creatorToken: usersAtas[i].address,
        lottery: lotteryAccount.publicKey,
        signer: users[i].publicKey,
        proceeds,
        appStats,
        owner: owner.publicKey,
        feeAccount: feeAccount.publicKey
      }).signers([users[i]]).rpc();

      console.log("user1 " + users[i].publicKey + " bought a ticket txn hash is ", tx1);
    }

    // balance = await connection.getBalance(user1.publicKey);
    // console.log("balance of user1 is ", balance / LAMPORTS_PER_SOL);

    const lotteryInfo = await program.account.lottery.fetch(lotteryAccount.publicKey);
    const ticketAmount = lotteryInfo.ticketAmount
    const leftTickets = lotteryInfo.leftTickets.length
    const ticketPrice = lotteryInfo.ticketPrice
    const buyers = lotteryInfo.buyers

    for (let i = 0; i < buyers.length; i++) {
      const buyer = buyers[i];
      console.log({
        participant: buyer.participant.toBase58(),
        tickets: Array.from(buyer.tickets)
      })
    }

    console.log({
      //lotteryInfo,
      //buyers,
      leftTickets,
      ticketAmount,
      ticketPrice: Number(ticketPrice.toString()) / Math.pow(10, 9)
    })

    const feeAccountTokenBalance = await connection.getTokenAccountBalance(feeAccountAta.address);
    console.log("feeAccount token balance is ", feeAccountTokenBalance.value.uiAmount);
  });


  it("Reveal winners", async () => {
    await sleep(2000)
    const tx = await program.methods.revealWinners().accounts({
      lottery: lotteryAccount.publicKey,
      clock: anchor.web3.SYSVAR_CLOCK_PUBKEY
    }).rpc().catch(e => console.log(e));
    const raffleInfo = await program.account.lottery.fetch(lotteryAccount.publicKey);
    console.log(raffleInfo);
    console.log({
      collected: raffleInfo.collected.toString(),
    });
    const lotteryInfo = await program.account.lottery.fetch(lotteryAccount.publicKey);
    console.log(lotteryInfo);
  });



  it("Claim prize", async () => {

    const [appStats, bump] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode('app-stats'),
        owner.publicKey.toBuffer()
      ],
      program.programId
    );

    const lotteryInfo = await program.account.lottery.fetch(lotteryAccount.publicKey);
    console.log(lotteryInfo);

    let winnersPublicKeys = lotteryInfo.winners.map(winner => winner.participant.toBase58());
    let winners = users.filter(user => winnersPublicKeys.includes(user.publicKey.toBase58()));
    console.log(winners);

    // let winner token balance before claiming
    const winnerToken = await getOrCreateAssociatedTokenAccount(
      connection,
      winners[0],
      mint,
      winners[0].publicKey
    )
    let balance = await connection.getTokenAccountBalance(winnerToken.address);
    console.log("winner token balance before claiming is ", balance.value.uiAmount);

    const [prize, prize_bump] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode('prize'),
        lotteryAccount.publicKey.toBuffer(),
      ],
      program.programId
    );

    const tx = await program.methods.claimPrize().accounts(
      {
        appStats,
        user: winners[0].publicKey,
        lottery: lotteryAccount.publicKey,
        prize,
        mint,
        userToken: winnerToken.address
      }
    ).signers([winners[0]]).rpc();

    balance = await connection.getTokenAccountBalance(winnerToken.address);
    console.log("winner token balance after claiming is ", balance.value.uiAmount);

    const raffleInfo = await program.account.lottery.fetch(lotteryAccount.publicKey);
    console.log(raffleInfo);
    //console.log(tx);
  })
})

async function airdropSol(connection: any, publicKey: PublicKey) {
  await connection.confirmTransaction(
    await connection.requestAirdrop(
      publicKey,
      1 * LAMPORTS_PER_SOL
    )
  );
}
