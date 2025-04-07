import * as core from "./global";

(core as any).emulatorWasmLoaded = (_initInput?: any): Promise<void> =>
  Promise.resolve();

export { TychoExecutor } from "./executor/Executor";

export { setSignWithGlobalId } from "./utils/sign";
