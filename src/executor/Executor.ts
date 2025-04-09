import type {
  ExecutorEmulationResult,
  ExecutorGetMethodArgs,
  ExecutorGetMethodResult,
  ExecutorRunTickTockArgs,
  ExecutorRunTransactionArgs,
  IExecutor,
} from "@ton/sandbox";
import { Cell, serializeTuple } from "@ton/core";

import {
  defaultConfig,
  defaultConfigSeqno,
  defaultGlobalId,
} from "../config/defaultConfig";
import type {
  EmulatorParams,
  EmulatorResponse,
  RunGetMethodParams,
  RunGetMethodResponse,
  OkResponse,
  ErrResponse,
  VersionInfo,
} from "../wasm/tycho_emulator.js";
import * as emulatorWasm from "../wasm/tycho_emulator.js";

type EmulatorWasm = typeof emulatorWasm;

export class TychoExecutor implements IExecutor {
  public static defaultGlobalId: number = defaultGlobalId;
  public static defaultConfigSeqno: number = defaultConfigSeqno;
  public static defaultConfig: Cell = Cell.fromBase64(defaultConfig);

  private emulator?: {
    ptr: number;
    config: string;
    verbosity: number;
  };

  constructor(private module: EmulatorWasm) {}

  static async create() {
    const ex = new TychoExecutor(emulatorWasm);
    return ex;
  }

  async runGetMethod(
    args: ExecutorGetMethodArgs
  ): Promise<ExecutorGetMethodResult> {
    const params: RunGetMethodParams = {
      code: args.code.toBoc().toString("base64"),
      data: args.data.toBoc().toString("base64"),
      verbosity: 0,
      libs: args.libs?.toBoc().toString("base64"),
      address: args.address.toString(),
      unixtime: args.unixTime,
      balance: args.balance.toString(),
      rand_seed: args.randomSeed.toString("hex"),
      gas_limit: args.gasLimit.toString(),
      method_id: args.methodId,
      debug_enabled: args.debugEnabled,
    };

    if (args.extraCurrency !== undefined) {
      params.extra_currencies = {};
      for (const [k, v] of Object.entries(args.extraCurrency)) {
        params.extra_currencies[k] = v.toString();
      }
    }

    let stack = serializeTuple(args.stack);

    const resp: OkResponse<RunGetMethodResponse> | ErrResponse = JSON.parse(
      this.module.run_get_method(
        JSON.stringify(params),
        stack.toBoc().toString("base64"),
        args.config
      )
    );

    if (resp.ok) {
      return {
        output: resp.output,
        logs: resp.logs,
        debugLogs: "",
      };
    } else {
      throw new Error(`Unknown emulation error: ${resp.message}`);
    }
  }

  async runTickTock(
    args: ExecutorRunTickTockArgs
  ): Promise<ExecutorEmulationResult> {
    const params: EmulatorParams = {
      ...runCommonArgsToInternalParams(args),
      is_tick_tock: true,
      is_tock: args.which === "tock",
    };

    return this.runCommon(
      this.getEmulatorPointer(args.config, 0),
      args.libs?.toBoc().toString("base64"),
      args.shardAccount,
      null,
      JSON.stringify(params)
    );
  }

  async runTransaction(
    args: ExecutorRunTransactionArgs
  ): Promise<ExecutorEmulationResult> {
    const params = runCommonArgsToInternalParams(args);

    return this.runCommon(
      this.getEmulatorPointer(args.config, 0),
      args.libs?.toBoc().toString("base64"),
      args.shardAccount,
      args.message.toBoc().toString("base64"),
      JSON.stringify(params)
    );
  }

  getVersion(): { commitHash: string; commitDate: string } {
    const v: VersionInfo = JSON.parse(this.module.version());

    return {
      commitHash: v.emulatorLibCommitHash,
      commitDate: v.emulatorLibCommitDate,
    };
  }

  private runCommon(
    ...args: Parameters<typeof emulatorWasm.emulate_with_emulator>
  ): ExecutorEmulationResult {
    const resp: OkResponse<EmulatorResponse> | ErrResponse = JSON.parse(
      this.module.emulate_with_emulator.apply(this, args)
    );

    if (!resp.ok) {
      throw new Error(`Unknown emulation error: ${resp.message}`);
    }

    const result = resp.output;

    return {
      result: result.success
        ? {
            success: true,
            transaction: result.transaction,
            shardAccount: result.shard_account,
            vmLog: result.vm_log,
            actions: result.actions,
          }
        : {
            success: false,
            error: result.error,
            vmResults:
              "vm_log" in result
                ? {
                    vmLog: result.vm_log,
                    vmExitCode: result.vm_exit_code,
                  }
                : undefined,
          },
      logs: resp.logs,
      debugLogs: "",
    };
  }

  private getEmulatorPointer(config: string, verbosity: number) {
    if (
      this.emulator === undefined ||
      verbosity !== this.emulator.verbosity ||
      config !== this.emulator.config
    ) {
      this.createEmulator(config, verbosity);
    }

    return this.emulator!.ptr;
  }

  private createEmulator(config: string, verbosity: number) {
    if (this.emulator !== undefined) {
      this.module.destroy_emulator(this.emulator.ptr);
    }

    const ptr = this.module.create_emulator(config, verbosity);
    this.emulator = {
      ptr,
      config,
      verbosity,
    };
  }
}

function runCommonArgsToInternalParams(
  args: ExecutorRunTransactionArgs | ExecutorRunTickTockArgs
): EmulatorParams {
  return {
    unixtime: args.now,
    lt: args.lt.toString(),
    rand_seed: args.randomSeed?.toString("hex"),
    ignore_chksig: args.ignoreChksig,
    debug_enabled: args.debugEnabled,
  };
}
