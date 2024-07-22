import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Lottery } from "../target/types/lottery";
import { PublicKey, LAMPORTS_PER_SOL, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { BN } from "bn.js";
import NodeWallet from "@coral-xyz/anchor/dist/cjs/nodewallet";
import { ASSOCIATED_TOKEN_PROGRAM_ID, createMint, getOrCreateAssociatedTokenAccount, mintTo, TOKEN_PROGRAM_ID } from "@solana/spl-token";


describe("Lottery", () => {

  function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms))
  }

  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();

  anchor.setProvider(provider);
  const connection = provider.connection;
  const program = anchor.workspace.Lottery as Program<Lottery>;
  const mint = new PublicKey('GDYJuvNDqueoXK6ACeNJcQjjyEjD5RnBfRLFnVAsuGvL');

  const owner = provider.wallet as NodeWallet;
  const user1 = anchor.web3.Keypair.generate();
  const user2 = anchor.web3.Keypair.generate();
  const lottery = anchor.web3.Keypair.generate();
  const feeAccount = new PublicKey('2xjaQvvUxLjdffPWjaaNnXp5aoCRMPhLtLxYPyZNnKQq');


  const mintAuthSC = anchor.web3.Keypair.generate();
  const mintKeypairSC = anchor.web3.Keypair.generate();
  let mintSC: PublicKey;
  let ownerAta;

  it("initalize", async () => {
    // const fee_percent = 5;
    // const [appStats, bump] = PublicKey.findProgramAddressSync(
    //   [
    //     anchor.utils.bytes.utf8.encode('app-stats'),
    //     owner.publicKey.toBuffer()
    //   ],
    //   program.programId
    // );

    // const tx = await program.methods.createAppStats(
    //   fee_percent,
    //   bump
    // ).accounts({
    //   appStats,
    //   feeAccount
    // }).rpc();
  });

  it("Init mintToken", async () => {


    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        user1.publicKey,
        2 * LAMPORTS_PER_SOL
      )
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        mintAuthSC.publicKey,
        2 * LAMPORTS_PER_SOL
      )
    );

    // Stablecoin mint
    mintSC = await createMint(
      provider.connection,
      user1,
      mintAuthSC.publicKey,
      mintAuthSC.publicKey,
      10,
      mintKeypairSC,
      undefined,
      TOKEN_PROGRAM_ID
    );

    // Initialise ATA
    ownerAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      user1,
      mintSC,
      owner.publicKey
    );

    // Top up test account with SPL
    await mintTo(
      provider.connection,
      user1,
      mintSC,
      ownerAta.address,
      mintAuthSC,
      100000000,
      [],
      undefined,
      TOKEN_PROGRAM_ID
    );

    // transfer tokens to user1
  })

  it("Create competition", async () => {
    //const price = new BN(1 * LAMPORTS_PER_SOL);
    const price = new BN(1);
    const ticketAmount = 2;
    const prizeAmount = new BN(100 * Math.pow(10, 9));
    const end = new BN(Date.now() + 1);

    const [prize, prize_bump] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode('prize'),
        lottery.publicKey.toBuffer(),
      ],
      program.programId
    );

    const [proceeds, proceeds_bump] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode('proceeds'),
        lottery.publicKey.toBuffer(),
      ],
      program.programId
    );


    const ixn1 = await program.methods.createLottery(
      price,
      ticketAmount,
      prizeAmount,
      end,
      prize_bump,
      proceeds_bump
    ).accounts({
      lottery: lottery.publicKey,
      mint,
      prize,
      proceeds
    }).signers([
      lottery,
      owner.payer
    ]).instruction();

    const creatorToken = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      owner.payer,
      mint,
      owner.publicKey
    );


    const ixn2 = await program.methods.addPrize().accounts({
      lottery: lottery.publicKey,
      creatorToken: creatorToken.address,
      prize,
      mint,
      signer: owner.publicKey
    }).instruction();



    const instructions = [ixn1, ixn2];

    const { blockhash } = await connection.getLatestBlockhash();

    const message = new TransactionMessage({
      payerKey: owner.publicKey,
      recentBlockhash: blockhash,
      instructions
    }).compileToV0Message();

    const transaction = new VersionedTransaction(message);

    transaction.sign([owner.payer, lottery]);

    const tx = await connection.sendTransaction(transaction);


    console.log("Creating Competion txn hash is ", tx);
  });

  // it("Buy tickets", async () => {
  //   const airdropTx1 = await connection.requestAirdrop(
  //     user1.publicKey,
  //     2 * LAMPORTS_PER_SOL
  //   );
  //   const airdropTx2 = await connection.requestAirdrop(
  //     user2.publicKey,
  //     2 * LAMPORTS_PER_SOL
  //   );
  //   await connection.confirmTransaction(airdropTx1);
  //   await connection.confirmTransaction(airdropTx2);


  //   const [proceeds, proceeds_bump] = PublicKey.findProgramAddressSync(
  //     [
  //       anchor.utils.bytes.utf8.encode('proceeds'),
  //       lottery.publicKey.toBuffer(),
  //     ],
  //     program.programId
  //   );

  //   const [appStats, bump] = PublicKey.findProgramAddressSync(
  //     [
  //       anchor.utils.bytes.utf8.encode('app-stats'),
  //       owner.publicKey.toBuffer()
  //     ],
  //     program.programId
  //   );

  //   let balance = await connection.getBalance(user1.publicKey);
  //   console.log(balance)

  //   const tx1 = await program.methods.buyTickets(
  //     1
  //   ).accounts({
  //     lottery: lottery.publicKey,
  //     signer: user1.publicKey,
  //     proceeds,
  //     appStats,
  //     owner: owner.publicKey,
  //     feeAccount
  //   }).signers([user1]).rpc();
  //   console.log("user1 bought a ticket txn hash is ", tx1);

  //   balance = await connection.getBalance(user1.publicKey);
  //   console.log(balance)

  //   const tx2 = await program.methods.buyTickets(
  //     1
  //   ).accounts({
  //     lottery: lottery.publicKey,
  //     signer: user2.publicKey,
  //     proceeds,
  //     appStats,
  //     owner: owner.publicKey,
  //     feeAccount
  //   }).signers([user2]).rpc();
  //   console.log("user2 bought a ticket txn hash is ", tx2);

  //   const raffleInfo = await program.account.raffle.fetch(lottery.publicKey);
  //   // console.log(raffleInfo.buyers.length);
  // });

  // it("Reveal winner", async () => {
  //   await sleep(2000)
  //   const tx = await program.methods.revealWinner().accounts({
  //     lottery: lottery.publicKey,
  //     clock: anchor.web3.SYSVAR_CLOCK_PUBKEY
  //   }).rpc().catch(e => console.log(e));
  //   const raffleInfo = await program.account.raffle.fetch(lottery.publicKey);
  //   console.log(tx);
  //   console.log('Winner is', raffleInfo.winner.toBase58());
  // });

  // it("Claim prize", async () => {
  //   const raffleInfo = await program.account.raffle.fetch(lottery.publicKey);
  //   let winner = user1;
  //   if (raffleInfo.winner.toBase58() === user1.publicKey.toBase58()) {
  //     winner = user1;
  //   } else {
  //     winner = user2;
  //   }

  //   const [prize, prize_bump] = PublicKey.findProgramAddressSync(
  //     [
  //       anchor.utils.bytes.utf8.encode('prize'),
  //       lottery.publicKey.toBuffer(),
  //     ],
  //     program.programId
  //   );

  //   const userToken = await getOrCreateAssociatedTokenAccount(
  //     connection,
  //     winner,
  //     mint,
  //     winner.publicKey
  //   )

  //   const tx = await program.methods.claimPrize().accounts(
  //     {
  //       user: winner.publicKey,
  //       lottery: lottery.publicKey,
  //       prize,
  //       mint,
  //       userToken: userToken.address
  //     }
  //   ).signers([winner]).rpc();
  //   console.log(tx);
  // })
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
