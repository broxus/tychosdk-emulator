import axios from "axios";
import { z } from "zod";
import { Address, AccountState, Cell, loadAccount } from "@ton/core";
import { Blockchain, BlockchainStorage, SmartContract } from "@ton/sandbox";

export class TychoRemoteBlockchainStorage implements BlockchainStorage {
  private contracts: Map<string, SmartContract> = new Map();
  private client: JrpcClient;

  constructor(params: { url: string }) {
    this.client = new JrpcClient(params.url);
  }

  async getContract(blockchain: Blockchain, address: Address) {
    let existing = this.contracts.get(address.toString());
    if (!existing) {
      let account = await this.client.getAccount(address);

      const lt = account.lastTransaction?.lt ?? 0n;

      existing = new SmartContract(
        {
          lastTransactionHash: BigInt(
            "0x" + (account.lastTransaction?.hash?.toString("hex") ?? "0")
          ),
          lastTransactionLt: lt,
          account: {
            addr: address,
            storageStats: {
              used: {
                cells: 0n,
                bits: 0n,
                publicCells: 0n,
              },
              lastPaid: 0,
              duePayment: null,
            },
            storage: {
              lastTransLt: lt === 0n ? 0n : lt + 1n,
              balance: { coins: account.balance },
              state: account.state,
            },
          },
        },
        blockchain
      );

      this.contracts.set(address.toString(), existing);
    }

    return existing;
  }

  knownContracts() {
    return Array.from(this.contracts.values());
  }

  clearKnownContracts() {
    this.contracts.clear();
  }
}

class JrpcClient {
  constructor(private url: string) {}

  async getAccount(address: Address): Promise<{
    state: AccountState;
    balance: bigint;
    lastTransaction?: { lt: bigint; hash: Buffer };
  }> {
    const res = await axios.request({
      url: this.url,
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      data: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "getContractState",
        params: {
          address: address.toRawString(),
        },
      }),
    });

    if (res.status != 200) {
      console.error(res.data);
      throw new Error(res.statusText);
    }

    const data = responseSchema.parse(res.data);
    if (data.result == null) {
      console.error(data.error);
      throw new Error("Bad response");
    }

    const parsed = accountSchema.parse(data.result);
    if (parsed.type === "exists") {
      const state = loadAccount(Cell.fromBase64(parsed.account).asSlice());
      return {
        state: state.storage.state,
        balance: state.storage.balance.coins,
        lastTransaction: {
          lt: BigInt(parsed.lastTransactionId.lt),
          hash: Buffer.from(parsed.lastTransactionId.hash, "hex"),
        },
      };
    } else if (parsed.type === "notExists") {
      return {
        balance: 0n,
        state: {
          type: "uninit",
        },
      };
    } else {
      throw new Error("Unknown account state");
    }
  }
}

const responseSchema = z.object({
  jsonrpc: z.literal("2.0"),
  result: z.any().optional(),
  error: z.any().optional(),
  id: z.union([z.number(), z.string(), z.null()]),
});

const timingsSchema = z.object({
  genLt: z.string(),
  genUtime: z.number(),
});

const lastTransactionIdSchema = z.object({
  lt: z.string(),
  hash: z.string(),
});

const accountSchema = z.union([
  z.object({
    type: z.literal("exists"),
    account: z.string(),
    timings: timingsSchema,
    lastTransactionId: lastTransactionIdSchema,
  }),
  z.object({
    type: z.literal("notExists"),
    timings: timingsSchema,
  }),
]);
