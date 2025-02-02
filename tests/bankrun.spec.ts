import { Connection, PublicKey } from "@solana/web3.js";
import { describe } from "node:test";
import { BanksClient, ProgramTestContext, startAnchor } from "solana-bankrun";
// import IDL from "../target/idl/lending.json";
import { Program } from "@coral-xyz/anchor";
import { PythSolanaReceiver } from "@pythnetwork/pyth-solana-receiver";
import { Keypair } from "@solana/web3.js";
import { BankrunProvider } from "anchor-bankrun";
import { BankrunContextWrapper } from "../bankrun-utils/bankrunConnection";
import { Lending } from "../target/types/lending";

const IDL = require("../target/idl/lending.json");

describe("Lending Smart Contract Tests", async () => {
  let context: ProgramTestContext;
  let provider: BankrunProvider;
  let bankrunContextWrapper: BankrunContextWrapper;
  let program: Program<Lending>;
  let banksClient: BanksClient;
  let signer: Keypair;

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
});
