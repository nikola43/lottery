import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Lottery } from "../target/types/lottery";
import { PublicKey, LAMPORTS_PER_SOL, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { BN } from "bn.js";
import NodeWallet from "@coral-xyz/anchor/dist/cjs/nodewallet";
import { Account, ASSOCIATED_TOKEN_PROGRAM_ID, createAssociatedTokenAccountIdempotent, createAssociatedTokenAccountIdempotentInstruction, createMint, getOrCreateAssociatedTokenAccount, mintTo, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { use } from "chai";


describe("Lottery", () => {

  function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms))
  }

  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();

  anchor.setProvider(provider);
  const connection = provider.connection;
  const program = anchor.workspace.Lottery as Program<Lottery>;
  //const mint = new PublicKey('GDYJuvNDqueoXK6ACeNJcQjjyEjD5RnBfRLFnVAsuGvL');
  //const feeAccount = new PublicKey('2xjaQvvUxLjdffPWjaaNnXp5aoCRMPhLtLxYPyZNnKQq');

  const owner = provider.wallet as NodeWallet;
  const users = Array.from({ length: 10 }, () => anchor.web3.Keypair.generate());
  const usersAtas: Account[] = [];

  const feeAccount = users[0];
  const user1 = users[1];
  const user2 = users[2];
  let winner;
  const lotteryAccount = anchor.web3.Keypair.generate();

  console.log({
    owner: owner.publicKey.toBase58(),
    user1: user1.publicKey.toBase58(),
    user2: user2.publicKey.toBase58(),
    feeAccount: feeAccount.publicKey.toBase58(),
    lotteryAccount: lotteryAccount.publicKey.toBase58()
  })


  const mintKeypairSC = anchor.web3.Keypair.generate();
  let mint = mintKeypairSC.publicKey;

  it("initalize", async () => {
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        owner.publicKey,
        2 * LAMPORTS_PER_SOL
      )
    );

    const fee_percent = 5;
    const [appStats, bump] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode('app-stats'),
        owner.publicKey.toBuffer()
      ],
      program.programId
    );

    const tx = await program.methods.createAppStats(
      fee_percent,
      bump
    ).accounts({
      appStats,
      feeAccount: feeAccount.publicKey,
    }).rpc();
  });

  it("Init mintToken", async () => {
    // Stablecoin mint
    await createMint(
      provider.connection,
      owner.payer,
      owner.payer.publicKey,
      owner.payer.publicKey,
      9,
      mintKeypairSC,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    console.log({
      mint: mint.toBase58(),
      mintKeypairSC: mintKeypairSC.publicKey.toBase58()
    })


    const sourceAccount = await createAssociatedTokenAccountIdempotent(connection, owner.payer, mint, owner.publicKey, {}, TOKEN_2022_PROGRAM_ID);
    const mintSig = await mintTo(connection, owner.payer, mint, sourceAccount, owner.payer, 1000000000 * LAMPORTS_PER_SOL, [], undefined, TOKEN_2022_PROGRAM_ID);
    const ownerTokenBalance = await connection.getTokenAccountBalance(sourceAccount);
    console.log(ownerTokenBalance.value.uiAmount);

    // createAssociatedTokenAccountIdempotentInstruction(
    //   owner.publicKey,
    //   ataOut,
    //   owner.publicKey,
    //   mint,
    // ),

    // // Initialise ATA
    // ownerAta = await getOrCreateAssociatedTokenAccount(
    //   provider.connection,
    //   owner.payer,
    //   mint,
    //   owner.publicKey
    // );

    // Top up test account with SPL
    // await mintTo(
    //   provider.connection,
    //   owner.payer,
    //   mint,
    //   ownerAta.address,
    //   owner.payer,
    //   1000000000 * LAMPORTS_PER_SOL,
    //   [],
    //   undefined,
    //   TOKEN_PROGRAM_ID
    // );
  })

  it("Should airdrop users", async () => {

    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        owner.publicKey,
        2 * LAMPORTS_PER_SOL
      )
    );

    for (let i = 0; i < users.length; i++) {
      // Airdrop SOL
      await provider.connection.confirmTransaction(
        await provider.connection.requestAirdrop(
          users[i].publicKey,
          2 * LAMPORTS_PER_SOL
        )
      );

      // Create ATA
      usersAtas[i] = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        users[i],
        mint,
        users[i].publicKey
      );

      // transfer 100 tokens to each user
      await mintTo(
        provider.connection,
        owner.payer,
        mint,
        usersAtas[i].address,
        owner.payer,
        10000000000, // 10 tokens
        [],
        undefined,
        TOKEN_PROGRAM_ID
      );
    }
  })

  it("Create competition", async () => {
    const ticketPrice = new BN(10 * Math.pow(10, 9)); // 10 tokens
    console.log(ticketPrice.toString());
    //const ticketPrice = new BN(1);
    const ticketAmount = 100;
    //console.log(prizeAmount.toString());
    const end = new BN(Date.now() + 1);

    const [prize, prize_bump] = PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("prize"), lotteryAccount.publicKey.toBuffer()],
      program.programId
    );

    const [proceeds, proceeds_bump] = PublicKey.findProgramAddressSync(
      [anchor.utils.bytes.utf8.encode("proceeds"), lotteryAccount.publicKey.toBuffer()],
      program.programId
    );

    const ixn1 = await program.methods.createLottery(
      ticketPrice,
      ticketAmount,
      end,
      prize_bump,
      proceeds_bump
    ).accounts({
      lottery: lotteryAccount.publicKey,
      mint,
      prize,
      proceeds
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
  });

  it("Should get lotteryInfo", async () => {
    const lotteryInfo = await program.account.lottery.fetch(lotteryAccount.publicKey);
    const ticketAmount = lotteryInfo.ticketAmount
    const leftTickets = lotteryInfo.leftTickets.length
    const ticketPrice = lotteryInfo.ticketPrice
    const end = lotteryInfo.end
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

    let balance = await connection.getBalance(user1.publicKey);
    //console.log(balance)

    for (let i = 1; i < users.length; i++) {
      const tx1 = await program.methods.buyTickets(
        new BN(1)
      ).accounts({
        creatorToken: usersAtas[i].address,
        prize,
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
    const end = lotteryInfo.end
    console.log({
      buyers,
      leftTickets,
      ticketAmount,
      ticketPrice: Number(ticketPrice.toString()) / Math.pow(10, 9)
    })
  });

  it("Reveal winner", async () => {
    await sleep(2000)
    const tx = await program.methods.revealWinner().accounts({
      lottery: lotteryAccount.publicKey,
      clock: anchor.web3.SYSVAR_CLOCK_PUBKEY
    }).rpc().catch(e => console.log(e));
    const raffleInfo = await program.account.lottery.fetch(lotteryAccount.publicKey);
    //console.log(tx);
    winner = users.find(user => user.publicKey.toBase58() === raffleInfo.winner.toBase58());
    console.log('Winner is', raffleInfo.winner.toBase58());
  });

  it("Claim prize", async () => {

    // let winner token balance before claiming
    const winnerToken = await getOrCreateAssociatedTokenAccount(
      connection,
      winner,
      mint,
      winner.publicKey
    )
    let balance = await connection.getTokenAccountBalance(winnerToken.address);
    console.log("winner token balance before claiming is ", balance.value.uiAmount);

    const raffleInfo = await program.account.lottery.fetch(lotteryAccount.publicKey);

    const [prize, prize_bump] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode('prize'),
        lotteryAccount.publicKey.toBuffer(),
      ],
      program.programId
    );

    const tx = await program.methods.claimPrize().accounts(
      {
        user: winner.publicKey,
        lottery: lotteryAccount.publicKey,
        prize,
        mint,
        userToken: winnerToken.address
      }
    ).signers([winner]).rpc();

    balance = await connection.getTokenAccountBalance(winnerToken.address);
    console.log("winner token balance after claiming is ", balance.value.uiAmount);
    //console.log(tx);
  })


  // it("Collect proceeds", async () => {
  //   let accounts = await program.account.raffle.all();
  //   console.log(accounts.length)
  //   const creatorToken = await getOrCreateAssociatedTokenAccount(
  //     provider.connection,
  //     owner.payer,
  //     mint,
  //     owner.publicKey
  //   );

  //   const [prize, prize_bump] = PublicKey.findProgramAddressSync(
  //     [
  //       anchor.utils.bytes.utf8.encode('prize'),
  //       lottery.publicKey.toBuffer(),
  //     ],
  //     program.programId
  //   );

  //   const [proceeds, proceeds_bump] = PublicKey.findProgramAddressSync(
  //     [
  //       anchor.utils.bytes.utf8.encode('proceeds'),
  //       lottery.publicKey.toBuffer(),
  //     ],
  //     program.programId
  //   );
  //   let balance = await connection.getBalance(owner.publicKey);
  //   console.log(balance / LAMPORTS_PER_SOL);
  //   const [appStats, bump] = PublicKey.findProgramAddressSync(
  //     [
  //       anchor.utils.bytes.utf8.encode('app-stats'),
  //       owner.publicKey.toBuffer()
  //     ],
  //     program.programId
  //   );
  //   const tx = await program.methods.collectProceed().accounts({
  //     lottery: lottery.publicKey,
  //     userToken: creatorToken.address,
  //     prize,
  //     proceeds,
  //     mint,
  //     creator: owner.publicKey,
  //     appStats,
  //     feeAccount,
  //     owner: owner.publicKey
  //   }).signers([owner.payer]).rpc()
  //   console.log(tx);
  //   balance = await connection.getBalance(owner.publicKey);
  //   console.log(balance / LAMPORTS_PER_SOL);
  //   accounts = await program.account.raffle.all();
  //   console.log(accounts.length)
  // });
});


