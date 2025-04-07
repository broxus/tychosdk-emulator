import * as global from "./global";
import init from "@tychosdk/emulator-wasm";

let wasmInitializationStarted = false;
let notifyWasmInitialized: { resolve: () => void; reject: () => void };
const initializationPromise: Promise<void> = new Promise<void>(
  (resolve, reject) => {
    notifyWasmInitialized = { resolve, reject };
  }
);

(global as any).emulatorWasmLoaded = (initInput?: any): Promise<void> => {
  if (!wasmInitializationStarted) {
    wasmInitializationStarted = true;
    init(initInput)
      .then(notifyWasmInitialized.resolve)
      .catch(notifyWasmInitialized.reject);
  }
  return initializationPromise;
};

export { TychoExecutor } from "./executor/Executor";

export { setSignWithGlobalId } from "./utils/sign";
