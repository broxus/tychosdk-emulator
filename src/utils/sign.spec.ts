import {
  getSecureRandomBytes,
  KeyPair,
  keyPairFromSeed,
  sign,
} from "@ton/crypto";
import { cryptoWithSignatureId, setSignatureId } from "./sign";

describe("setSignWithGlobalId", () => {
  const data = Buffer.from("Hello wordl!");
  let keypair: KeyPair;
  let targetSignature: Buffer<ArrayBufferLike>;

  beforeAll(async () => {
    const seed = await getSecureRandomBytes(32);
    keypair = keyPairFromSeed(seed);
    targetSignature = sign(data, keypair.secretKey);
  });

  it("should not break on empty globalId", () => {
    const crypto = cryptoWithSignatureId(undefined);

    const newSignature = crypto.sign(data, keypair.secretKey);
    expect(newSignature.equals(targetSignature)).toBe(true);
    expect(crypto.signVerify(data, newSignature, keypair.publicKey)).toBe(true);
  });

  it("should work when a globalId set", () => {
    const oldCrypto = cryptoWithSignatureId(undefined);
    const newCrypto = cryptoWithSignatureId(123);

    const newSignature = newCrypto.sign(data, keypair.secretKey);
    expect(newSignature.equals(targetSignature)).toBe(false);
    expect(newCrypto.signVerify(data, newSignature, keypair.publicKey)).toBe(
      true
    );
    expect(newCrypto.signVerify(data, targetSignature, keypair.publicKey)).toBe(
      false
    );

    expect(oldCrypto.signVerify(data, newSignature, keypair.publicKey)).toBe(
      false
    );
    expect(oldCrypto.signVerify(data, targetSignature, keypair.publicKey)).toBe(
      true
    );
  });

  it("should properly update signature id", () => {
    const newCrypto = cryptoWithSignatureId(345);
    setSignatureId(newCrypto, undefined);

    const newSignature = newCrypto.sign(data, keypair.secretKey);
    expect(newSignature.equals(targetSignature)).toBe(true);
    expect(newCrypto.signVerify(data, newSignature, keypair.publicKey)).toBe(
      true
    );
  });

  it("should handle negative global ids", () => {
    const crypto = cryptoWithSignatureId(-6001);

    const newSignature = crypto.sign(data, keypair.secretKey);
    expect(newSignature.equals(targetSignature)).toBe(false);
    expect(crypto.signVerify(data, newSignature, keypair.publicKey)).toBe(true);
  });
});
