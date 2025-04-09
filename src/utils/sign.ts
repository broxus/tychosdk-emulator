import * as tonCrypto from "@ton/crypto";

const originalTonCrypto = { ...tonCrypto };

export function setSignWithGlobalId(globalId: number | undefined) {
  if (globalId != null) {
    let globalIdBytes = Buffer.alloc(4);
    globalIdBytes.writeInt32BE(globalId);

    Object.defineProperty(tonCrypto, "sign", {
      get: function () {
        return function (...args: any[]) {
          args[0] = Buffer.concat([globalIdBytes, args[0]]);
          // @ts-ignore
          return originalTonCrypto.sign.apply(this, args);
        };
      },
    });
  } else {
    Object.defineProperty(tonCrypto, "sign", {
      get: function () {
        return originalTonCrypto.sign.bind(this);
      },
    });
  }
}
