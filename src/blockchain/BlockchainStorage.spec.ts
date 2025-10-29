import { TychoRemoteBlockchainStorage } from "./BlockchainStorage";
import { TychoExecutor } from "../executor/Executor";
import { Blockchain } from "@ton/sandbox";
import { address, beginCell, storeShardAccount } from "@ton/core";

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

    const res = await blockchain.runGetMethod(
      electorAddress,
      "past_election_ids"
    );
    expect(res.exitCode).toBe(0);
    expect(res.gasUsed).toBeGreaterThan(0);
  });

  it("should get token contract", async () => {
    const contractAddress = address(
      "0:5ee27bd184049818ff87ff88d25867c47a5d24f38ae40852da17f0b6d51e990d"
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
});
