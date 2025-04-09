import * as tonCrypto from "@ton/crypto";

const originalTonCrypto = { ...tonCrypto };

export function cryptoWithSignatureId(
  globalId: number | null | undefined
): typeof tonCrypto {
  const result = { ...originalTonCrypto };
  setSignatureId(result, globalId);
  return result;
}

export function setSignatureId(
  crypto: typeof tonCrypto,
  globalId: number | null | undefined
) {
  if (globalId != null) {
    let globalIdBytes = Buffer.alloc(4);
    globalIdBytes.writeInt32BE(globalId);

    Object.defineProperty(crypto, "sign", {
      get: function () {
        return function (...args: any[]) {
          args[0] = Buffer.concat([globalIdBytes, args[0]]);
          // @ts-ignore
          return originalTonCrypto.sign.apply(this, args);
        };
      },
    });

    Object.defineProperty(crypto, "signVerify", {
      get: function () {
        return function (...args: any[]) {
          args[0] = Buffer.concat([globalIdBytes, args[0]]);
          // @ts-ignore
          return originalTonCrypto.signVerify.apply(this, args);
        };
      },
    });
  } else {
    Object.defineProperty(crypto, "sign", {
      get: function () {
        return originalTonCrypto.sign.bind(this);
      },
    });

    Object.defineProperty(crypto, "signVerify", {
      get: function () {
        return originalTonCrypto.signVerify.bind(this);
      },
    });
  }
}
