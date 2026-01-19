import * as tonCrypto from "@ton/crypto";

const originalTonCrypto = { ...tonCrypto };

export function cryptoWithSignatureId(
  globalId: number | null | undefined,
): typeof tonCrypto {
  const result = { ...originalTonCrypto };
  setSignatureId(result, globalId);
  return result;
}

export function setSignatureId(
  crypto: typeof tonCrypto,
  globalId: number | null | undefined,
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

// === Signature domain impl ===

export type SignatureDomain =
  | { type: "empty" }
  | { type: "l2"; globalId: number };

export const TL_ID_SIGNATURE_DOMAIN_L2 = 0x71b34ee1;
export const TL_ID_SIGNATURE_DOMAIN_EMPTY = 0xe1d571b;

export const SIGNATURE_DOMAIN_EMPTY_HASH: Buffer = (() => {
  const tl = Buffer.alloc(4);
  tl.writeInt32LE(TL_ID_SIGNATURE_DOMAIN_EMPTY);
  return originalTonCrypto.sha256_sync(tl);
})();

export function signatureDomainPrefix(
  domain: SignatureDomain | Buffer,
): Buffer | null {
  if (Buffer.isBuffer(domain)) {
    if (domain.length !== 32) {
      throw new Error("invalid signature domain hash");
    } else if (domain.compare(SIGNATURE_DOMAIN_EMPTY_HASH) === 0) {
      return null;
    } else {
      return domain;
    }
  } else {
    switch (domain.type) {
      case "empty":
        return null;
      case "l2": {
        const tl = Buffer.alloc(8);
        tl.writeInt32LE(TL_ID_SIGNATURE_DOMAIN_L2);
        tl.writeInt32LE(domain.globalId, 4);
        return originalTonCrypto.sha256_sync(tl);
      }
      default:
        throw new Error("unknown signature domain type");
    }
  }
}

export function cryptoWithSignatureDomain(
  domain: SignatureDomain | Buffer | null | undefined,
): typeof tonCrypto {
  const result = { ...originalTonCrypto };
  setSignatureDomain(result, domain);
  return result;
}

export function setSignatureDomain(
  crypto: typeof tonCrypto,
  domain: SignatureDomain | Buffer | null | undefined,
) {
  const prefix = domain != null ? signatureDomainPrefix(domain) : null;

  if (prefix != null) {
    Object.defineProperty(crypto, "sign", {
      get: function () {
        return function (...args: any[]) {
          args[0] = Buffer.concat([prefix, args[0]]);
          // @ts-ignore
          return originalTonCrypto.sign.apply(this, args);
        };
      },
    });

    Object.defineProperty(crypto, "signVerify", {
      get: function () {
        return function (...args: any[]) {
          args[0] = Buffer.concat([prefix, args[0]]);
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
