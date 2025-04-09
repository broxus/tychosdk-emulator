import fs from "fs";
import axios from "axios";
import { z } from "zod";
import { Cell } from "@ton/core";

function parseConfigRoot(config: Cell): Cell {
  const cs = config.asSlice();
  cs.loadBuffer(32);
  return cs.loadRef();
}

function writeConfig(
  name: string,
  config: Cell,
  seqno: number,
  globalId: number
) {
  const configBoc = config.toBoc({ idx: false }).toString("base64");

  const out = `export const ${name}GlobalId = ${globalId};
export const ${name}ConfigSeqno = ${seqno};
export const ${name}Config = '${configBoc}';`;

  fs.writeFileSync(`./src/config/${name}Config.ts`, out);
}

const responseSchema = z.object({
  jsonrpc: z.literal("2.0"),
  result: z.any().optional(),
  error: z.any().optional(),
  id: z.union([z.number(), z.string(), z.null()]),
});

const blockchainConfigSchema = z.object({
  globalId: z.number(),
  seqno: z.number(),
  config: z.string(),
});

const main = async () => {
  const response = await axios
    .request({
      url: "https://rpc-testnet.tychoprotocol.com",
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      data: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "getBlockchainConfig",
        params: {},
      }),
    })
    .then((res) => {
      if (res.status != 200) {
        console.log(res.data);
        throw new Error(res.statusText);
      }

      const data = responseSchema.parse(res.data);
      if (data.result == null) {
        console.log(data.error);
        throw new Error("Bad response");
      }

      return blockchainConfigSchema.parse(data.result);
    });

  const configCell = Cell.fromBase64(response.config);
  writeConfig(
    "default",
    parseConfigRoot(configCell),
    response.seqno,
    response.globalId
  );
};

main();
