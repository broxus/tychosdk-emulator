import { TychoRemoteBlockchainStorage } from "./BlockchainStorage";
import { TychoExecutor } from "../executor/Executor";
import { Blockchain, createEmptyShardAccount } from "@ton/sandbox";
import { Address, address, Cell, loadMessage } from "@ton/core";

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
      "-1:3333333333333333333333333333333333333333333333333333333333333333"
    );

    const res = await blockchain.runGetMethod(electorAddress, "past_elections");
    expect(res.exitCode).toBe(0);
    expect(res.gasUsed).toBeGreaterThan(0);
  });
});
