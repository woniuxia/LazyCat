import {
  constants,
  createCipheriv,
  createDecipheriv,
  privateDecrypt,
  publicEncrypt,
  type BinaryLike
} from "node:crypto";

export function rsaEncrypt(plainText: string, publicKeyPem: string): string {
  const encrypted = publicEncrypt(
    {
      key: publicKeyPem,
      padding: constants.RSA_PKCS1_OAEP_PADDING
    },
    Buffer.from(plainText, "utf8")
  );
  return encrypted.toString("base64");
}

export function rsaDecrypt(cipherTextBase64: string, privateKeyPem: string, passphrase?: string): string {
  const decrypted = privateDecrypt(
    {
      key: privateKeyPem,
      passphrase,
      padding: constants.RSA_PKCS1_OAEP_PADDING
    },
    Buffer.from(cipherTextBase64, "base64")
  );
  return decrypted.toString("utf8");
}

type AesAlgorithm = "aes-128-cbc" | "aes-192-cbc" | "aes-256-cbc";

export function aesEncrypt(plainText: string, key: BinaryLike, iv: BinaryLike, algorithm: AesAlgorithm = "aes-256-cbc"): string {
  const cipher = createCipheriv(algorithm, normalizeBuffer(key), normalizeBuffer(iv));
  return Buffer.concat([cipher.update(plainText, "utf8"), cipher.final()]).toString("base64");
}

type DesAlgorithm = "des-cbc" | "des-ede3-cbc";

export function desEncrypt(plainText: string, key: BinaryLike, iv: BinaryLike, algorithm: DesAlgorithm = "des-ede3-cbc"): string {
  const cipher = createCipheriv(algorithm, normalizeBuffer(key), normalizeBuffer(iv));
  return Buffer.concat([cipher.update(plainText, "utf8"), cipher.final()]).toString("base64");
}

export function desDecrypt(cipherTextBase64: string, key: BinaryLike, iv: BinaryLike, algorithm: DesAlgorithm = "des-ede3-cbc"): string {
  const decipher = createDecipheriv(algorithm, normalizeBuffer(key), normalizeBuffer(iv));
  return Buffer.concat([decipher.update(Buffer.from(cipherTextBase64, "base64")), decipher.final()]).toString("utf8");
}

export function aesDecrypt(cipherTextBase64: string, key: BinaryLike, iv: BinaryLike, algorithm: AesAlgorithm = "aes-256-cbc"): string {
  const decipher = createDecipheriv(algorithm, normalizeBuffer(key), normalizeBuffer(iv));
  return Buffer.concat([decipher.update(Buffer.from(cipherTextBase64, "base64")), decipher.final()]).toString("utf8");
}

function normalizeBuffer(input: BinaryLike): Buffer {
  if (typeof input === "string") {
    return Buffer.from(input, "utf8");
  }
  return Buffer.from(input as Buffer);
}
