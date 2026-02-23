import { createTestLogger } from "../logger";

// Fix BigInt serialization for Jest worker communication (Node.js v25+)
// Without this, Jest crashes with "TypeError: Do not know how to serialize a BigInt"
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(BigInt.prototype as any).toJSON = function () {
  return this.toString();
};

global.logger = createTestLogger();
