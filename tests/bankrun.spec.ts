import { Connection, PublicKey } from "@solana/web3.js";
import { describe } from "node:test";
import { BanksClient, ProgramTestContext, startAnchor } from "solana-bankrun";
import IDL from "../target/idl/lending.json";
import { Program } from "@coral-xyz/anchor";
import { PythSolanaReceiver } from "@pythnetwork/pyth-solana-receiver";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { Keypair } from "@solana/web3.js";
import { BankrunProvider } from "anchor-bankrun";
import { BN } from "bn.js";
// import { createMint, mintTo,createAccount } from "spl-token-bankrun";
import * as SplTokenBankrun from "spl-token-bankrun";
import { BankrunContextWrapper } from "../bankrun-utils/bankrunConnection";
import { Lending } from "../target/types/lending";

// const IDL = require("../target/idl/lending.json");

describe("Lending Smart Contract Tests", async () => {
  let context: ProgramTestContext;
  let provider: BankrunProvider;
  let bankrunContextWrapper: BankrunContextWrapper;
  let program: Program<Lending>;
  let banksClient: BanksClient;
  let signer: Keypair;
  let usdcBankAccount:PublicKey; 
  let solBankAccount:PublicKey;

  console.log("start to test ====")

  const pyth = new PublicKey("7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE");

  const devnetConnection = new Connection("https://api.devnet.solana.com");

  const accountInfo = await devnetConnection.getAccountInfo(pyth);

  context = await startAnchor(
    "",
    [{ name: "lending", programId: new PublicKey(IDL.address) }],
    [{ address: pyth, info: accountInfo }]
  );

  provider = new BankrunProvider(context);

  const SOL_PRICE_FEED_ID =
    "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";

  bankrunContextWrapper = new BankrunContextWrapper(context);

  const connection = bankrunContextWrapper.connection.toConnection();

  const pythSolanaReceiver = new PythSolanaReceiver({
    connection,
    wallet: provider.wallet,
  });

  const solUsdPriceFeedAccountAddress =
    pythSolanaReceiver.getPriceFeedAccountAddress(0, SOL_PRICE_FEED_ID);

  const feedAccountInfo = await devnetConnection.getAccountInfo(
    solUsdPriceFeedAccountAddress
  );

  context.setAccount(solUsdPriceFeedAccountAddress, feedAccountInfo);

  program = new Program<Lending>(IDL as Lending, provider);
  banksClient = context.banksClient;
  signer = provider.wallet.payer;

  const mintUSDC = await SplTokenBankrun.createMint(banksClient,signer,signer.publicKey,null,2);
  
  const mintSOL = await SplTokenBankrun.createMint(banksClient,signer,signer.publicKey,null,2);

  [usdcBankAccount] = PublicKey.findProgramAddressSync([
    Buffer.from("treasury"),
    mintUSDC.toBuffer()
  ],
  program.programId);

  [solBankAccount] = PublicKey.findProgramAddressSync([
    Buffer.from("treasury"),
    mintSOL.toBuffer()
  ],
  program.programId);

  it("Test Init And Fund Bank",async ()=>{
    const initUsdcBankTx = await program.methods.initBank(new BN(1),new BN(1)).accounts({
      signer:signer.publicKey,
      mint:mintUSDC,
      tokenProgram:TOKEN_PROGRAM_ID
    }).rpc({commitment:"confirmed"});

    console.log("Create Usdc Bank Account",initUsdcBankTx);

    const amount = 10_000 * 10 ** 9;

    const mintTx = await SplTokenBankrun.mintTo(
      banksClient,
      signer,
      mintUSDC,
      usdcBankAccount,
      signer,
      amount
    );

    console.log("Mint Usdc To Bank",mintTx);
  });

  it("Test Init User",async () => {
    const initUserTx = await program.methods.initUser(mintUSDC).accounts({
      signer:signer.publicKey
    }).rpc({commitment:"confirmed"});

    console.log("Init User:",initUserTx);
  });

  it("Test Init and Fund Sol Bank",async ()=> {
    const initSolBankTx = await program.methods.initBank(
      new BN(1),
      new BN(2)
    ).accounts({
      signer:signer.publicKey,
      mint:mintSOL,
      tokenProgram:TOKEN_PROGRAM_ID
    }).rpc({commitment:"confirmed"});

    console.log("Create Sol Bank Account",initSolBankTx);

    const amount = 10_000 * 10 ** 9;

    const mintTx = await SplTokenBankrun.mintTo(
      banksClient,
      signer,
      mintSOL,
      solBankAccount,
      signer,
      amount
    );

    console.log("Mint SOL To Bank",mintTx);
  });

  it("Create And Fund Token Accounts",async ()=> {
    const usdcTokenAccount = await SplTokenBankrun.createAccount(
      banksClient,
      signer,
      mintUSDC,
      signer.publicKey
    );

    console.log("USDC Token Account",usdcTokenAccount);

    const amount = 10_000 * 10 ** 9;

    const mintUSDCTx = await SplTokenBankrun.mintTo(
      banksClient,
      signer,
      mintUSDC,
      usdcTokenAccount,
      signer,
      amount
    );

    console.log("Mint USDC To Token Account",mintUSDCTx);
  });

  it("Test Deposit",async ()=> {
    const depositUsdcTx = await program.methods
    .deposit(new BN(100_000_000_000))
    .accounts({
      signer:signer.publicKey,
      mint:mintUSDC,
      tokenProgram:TOKEN_PROGRAM_ID
    }).rpc({commitment:"confirmed"});

    console.log("Deposti Usdc",depositUsdcTx);
  });

  it("Test Borrow",async () => {
    const borrowSOL = await program.methods.borrow(new BN(1)).accounts({
      signer:signer.publicKey,
      borrowMint:mintSOL,
      tokenProgram:TOKEN_PROGRAM_ID,
      priceUpdate:solUsdPriceFeedAccountAddress
    }).rpc({commitment:"confirmed"});

    console.log("Borrow SOL",borrowSOL);
  });

  it("Test Repay",async () => {
    const repaySol = await program.methods.repay(new BN(1)).accounts({
      signer:signer.publicKey,
      repayMint:mintSOL,
      tokenProgram:TOKEN_PROGRAM_ID
    }).rpc({commitment:"confirmed"});

    console.log("Repay Sol",repaySol);
  });

  it("Test Withdraw",async () => {
    const withdrawUsdc = await program.methods.withdraw(new BN(100)).accounts({
      signer:signer.publicKey,
      mint:mintUSDC,
      tokenProgram:TOKEN_PROGRAM_ID
    }).rpc({commitment:"confirmed"});

    console.log("Withdraw Usdc",withdrawUsdc);
  });

});
