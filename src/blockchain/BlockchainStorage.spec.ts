import assert from "assert";
import { TychoRemoteBlockchainStorage } from "./BlockchainStorage";
import { TychoExecutor } from "../executor/Executor";
import { Blockchain, createShardAccount } from "@ton/sandbox";
import { address, beginCell, Cell, Dictionary } from "@ton/core";
import { SignatureDomain, cryptoWithSignatureDomain } from "../utils/sign";

describe("TychoRemoteBlockchainStorage", () => {
  let blockchain: Blockchain;

  beforeAll(async () => {
    blockchain = await Blockchain.create({
      executor: await TychoExecutor.create(),
      config: TychoExecutor.defaultConfig,
      storage: new TychoRemoteBlockchainStorage({
        url: "https://rpc-testnet.tychoprotocol.com",
      }),
    });
  });

  it("should get elector contract", async () => {
    const electorAddress = address(
      "-1:3333333333333333333333333333333333333333333333333333333333333333",
    );

    const res = await blockchain.runGetMethod(
      electorAddress,
      "past_election_ids",
    );
    expect(res.exitCode).toBe(0);
    expect(res.gasUsed).toBeGreaterThan(0);
  });

  it("should get token contract", async () => {
    const contractAddress = address(
      "0:5ee27bd184049818ff87ff88d25867c47a5d24f38ae40852da17f0b6d51e990d",
    );

    const state = await blockchain.getContract(contractAddress);
    // const cell = beginCell()
    //   .store(storeShardAccount(state.account))
    //   .endCell()
    //   .toBoc()
    //   .toString("base64");
    // console.log(cell);

    const res = await state.get("get_wallet_data");
    expect(res.exitCode).toBe(0);
    expect(res.gasUsed).toBeGreaterThan(0);
  });

  it("should verify signature domain", async () => {
    const globalId = 2000;

    const params = Dictionary.loadDirect(
      Dictionary.Keys.Uint(32),
      Dictionary.Values.Cell(),
      TychoExecutor.defaultConfig,
    );
    const globalVersion = params.get(8)!.asSlice();
    const tag = globalVersion.loadUint(8);
    assert(tag == 0xc4);
    const version = globalVersion.loadUint(32);
    const capabilities =
      (globalVersion.loadUintBig(64) & ~0x4000000n) | 0x800000000n;

    const newGlobalVersion = beginCell()
      .storeUint(tag, 8)
      .storeUint(version, 32)
      .storeUint(capabilities, 64)
      .endCell();
    params.set(8, newGlobalVersion);
    params.set(19, beginCell().storeInt(globalId, 32).endCell());

    const blockchain = await Blockchain.create({
      executor: await TychoExecutor.create(),
      config: beginCell().storeDictDirect(params).endCell(),
    });
    // blockchain.verbosity = {
    //   vmLogs: "vm_logs_full",
    //   blockchainLogs: true,
    //   debugLogs: true,
    //   print: true,
    // };

    const addr = address(
      "0:0000000000000000000000000000000000000000000000000000000000000000",
    );
    await blockchain.setShardAccount(
      addr,
      createShardAccount({
        address: addr,
        balance: 1000n,
        // DROP
        // CHKSIGNS
        // 100 THROWIFNOT
        code: Cell.fromBase64("te6ccgEBAQEACAAADDD5EfLgZA=="),
        data: Cell.EMPTY,
      }),
    );

    const domain: SignatureDomain = { type: "l2", globalId };
    const myCrypto = cryptoWithSignatureDomain(domain);

    const seed = await myCrypto.getSecureRandomBytes(32);
    const keypair = myCrypto.keyPairFromSeed(seed);

    const data = Buffer.from("Hello world!");
    const newSignature = myCrypto.sign(data, keypair.secretKey);

    const res = await blockchain.runGetMethod(addr, -1, [
      {
        type: "slice",
        cell: beginCell().storeBuffer(data).endCell(),
      },
      {
        type: "slice",
        cell: beginCell().storeBuffer(newSignature, 64).endCell(),
      },
      {
        type: "int",
        value: BigInt(`0x${keypair.publicKey.toString("hex")}`),
      },
    ]);
    expect(res.exitCode).toBe(0);
  });
});
