import {
  Address,
  beginCell,
  Cell,
  contractAddress,
  Dictionary,
  loadTransaction,
  storeMessage,
  storeShardAccount,
  toNano,
  TransactionComputeVm,
} from "@ton/core";

import { defaultConfig } from "../config/defaultConfig";
import { TychoExecutor } from "./Executor";
import { createShardAccount } from "@ton/sandbox";

describe("executor", () => {
  let executor: TychoExecutor;
  beforeAll(async () => {
    executor = await TychoExecutor.create();
  });

  it("should run get method", async () => {
    let code = Cell.fromBase64(
      "te6ccsEBAgEAEQANEQEU/wD0pBP0vPLICwEABNOgu3u26g=="
    );
    let data = beginCell().endCell();

    let res = await executor.runGetMethod({
      verbosity: "full_location",
      code,
      data,
      address: contractAddress(0, { code, data }),
      config: defaultConfig,
      methodId: 0,
      stack: [
        { type: "int", value: 1n },
        { type: "int", value: 2n },
      ],
      balance: 0n,
      gasLimit: 0n,
      unixTime: 0,
      randomSeed: Buffer.alloc(32),
      debugEnabled: true,
    });
    expect(res.output.success).toBe(true);
  });

  it("should run transaction", async () => {
    let res = await executor.runTransaction({
      config: defaultConfig,
      libs: null,
      verbosity: "full_location",
      shardAccount: beginCell()
        .store(
          storeShardAccount({
            account: null,
            lastTransactionHash: 0n,
            lastTransactionLt: 0n,
          })
        )
        .endCell()
        .toBoc()
        .toString("base64"),
      message: beginCell()
        .store(
          storeMessage({
            info: {
              type: "internal",
              src: new Address(0, Buffer.alloc(32)),
              dest: new Address(0, Buffer.alloc(32)),
              value: { coins: 1000000000n },
              bounce: false,
              ihrDisabled: true,
              ihrFee: 0n,
              bounced: false,
              forwardFee: 0n,
              createdAt: 0,
              createdLt: 0n,
            },
            body: new Cell(),
          })
        )
        .endCell(),
      now: 0,
      lt: 0n,
      randomSeed: Buffer.alloc(32),
      ignoreChksig: false,
      debugEnabled: true,
    });
    expect(res.result.success).toBe(true);
  });

  it("can find library for get method", async () => {
    const libsDict = Dictionary.empty(
      Dictionary.Keys.Buffer(32),
      Dictionary.Values.Cell()
    );
    libsDict.set(Buffer.alloc(32, 0), new Cell());

    let code = Cell.fromBase64("te6ccgEBAQEAEwAAIshyzwsHgQEAz0BxzyPXOvgA");
    let data = beginCell().endCell();

    let res = await executor.runGetMethod({
      verbosity: "full_location",
      code,
      data,
      address: contractAddress(0, { code, data }),
      config: defaultConfig,
      methodId: 0,
      stack: [
        { type: "int", value: 1n },
        { type: "int", value: 2n },
      ],
      balance: 0n,
      gasLimit: 0n,
      unixTime: 0,
      randomSeed: Buffer.alloc(32),
      debugEnabled: true,
      libs: beginCell().storeDictDirect(libsDict).endCell(),
    });

    expect(res.output.success).toBe(true);
    if (res.output.success) {
      expect(res.output.vm_exit_code).toBe(0);
    }
  });

  it("can find library for executor", async () => {
    const libsDict = Dictionary.empty(
      Dictionary.Keys.Buffer(32),
      Dictionary.Values.Cell()
    );
    libsDict.set(Buffer.alloc(32, 0), new Cell());

    const account = createShardAccount({
      address: new Address(0, Buffer.alloc(32)),
      code: Cell.fromBase64("te6ccgEBAQEAEwAAIshyzwsHgQEAz0BxzyPXOvgA"),
      data: new Cell(),
      balance: toNano("1"),
    });

    let res = await executor.runTransaction({
      config: defaultConfig,
      libs: beginCell().storeDictDirect(libsDict).endCell(),
      verbosity: "full_location",
      shardAccount: beginCell()
        .store(storeShardAccount(account))
        .endCell()
        .toBoc()
        .toString("base64"),
      message: beginCell()
        .store(
          storeMessage({
            info: {
              type: "internal",
              src: new Address(0, Buffer.alloc(32)),
              dest: new Address(0, Buffer.alloc(32)),
              value: { coins: 1000000000n },
              bounce: false,
              ihrDisabled: true,
              ihrFee: 0n,
              bounced: false,
              forwardFee: 0n,
              createdAt: 0,
              createdLt: 0n,
            },
            body: new Cell(),
          })
        )
        .endCell(),
      now: 0,
      lt: 0n,
      randomSeed: Buffer.alloc(32),
      ignoreChksig: false,
      debugEnabled: true,
    });

    expect(res.result.success).toBe(true);
    if (res.result.success) {
      const tx = loadTransaction(
        Cell.fromBase64(res.result.transaction).asSlice()
      );

      expect(tx.description.type).toBe("generic");
      if (tx.description.type == "generic") {
        expect(tx.description.computePhase.type).toBe("vm");
        expect(
          (tx.description.computePhase as TransactionComputeVm).exitCode
        ).toBe(0);
      }
    }
  });

  it("reports version", () => {
    const v = executor.getVersion();
    expect(typeof v.commitHash).toBe("string");
    expect(typeof v.commitDate).toBe("string");
  });
});
